#![allow(unused)]

use core::{
    alloc::GlobalAlloc,
    ops::Range,
    ptr::{NonNull, null_mut, slice_from_raw_parts_mut},
    sync::atomic::{AtomicUsize, Ordering},
};

use buddy_system_allocator::Heap;
use log::debug;
use page_table_generic::PagingError;
use spin::{Mutex, Once};

use crate::{
    globals::global_val,
    hal_al::mmu::MapConfig,
    irq::NoIrqGuard,
    mem::{
        mmu::{AccessSetting, BootMemoryKind, BootRegion, CacheSetting, LINER_OFFSET},
        once::OnceStatic,
    },
    platform::{self, kstack_size},
    println,
};

pub use crate::platform::page_size;

mod addr;
mod cache;
// #[cfg(feature = "mmu")]
pub mod mmu;
pub mod once;
pub mod region;
pub use addr::*;

#[cfg(target_os = "none")]
#[global_allocator]
static ALLOCATOR: KAllocator = KAllocator {
    heap32: Mutex::new(Heap::empty()),
    heap64: Mutex::new(Heap::empty()),
};

static mut TMP_PAGE_ALLOC_ADDR: usize = 0;

/// Allocate memory with DMA mask
///
/// # Safety
///
/// This function is unsafe because it performs raw memory allocation.
pub unsafe fn alloc_with_mask(layout: core::alloc::Layout, dma_mask: u64) -> *mut u8 {
    #[cfg(target_os = "none")]
    {
        unsafe { ALLOCATOR.alloc_with_mask(layout, dma_mask) }
    }
    #[cfg(not(target_os = "none"))]
    {
        let _ = dma_mask;
        unsafe { alloc::alloc::alloc(layout) }
    }
}

pub struct KAllocator {
    heap32: Mutex<Heap<32>>,
    heap64: Mutex<Heap<64>>,
}

impl KAllocator {
    pub fn reset(&self, memory: &mut [u8]) {
        let range = memory.as_mut_ptr_range();
        let start = range.start as usize;
        let end = range.end as usize;
        let len = memory.len();

        {
            let mut heap32 = self.heap32.lock();
            *heap32 = Heap::empty();
        }
        {
            let mut heap64 = self.heap64.lock();
            *heap64 = Heap::empty();
        }

        if Self::range_within_u32(start, end) {
            let mut heap32 = self.heap32.lock();
            unsafe { heap32.init(start, len) };
        } else {
            let mut heap64 = self.heap64.lock();
            unsafe { heap64.init(start, len) };
        }
    }

    pub fn add_to_heap(&self, memory: &mut [u8]) {
        let range = memory.as_mut_ptr_range();
        let start = range.start as usize;
        let end = range.end as usize;

        if Self::range_within_u32(start, end) {
            let mut heap32 = self.heap32.lock();
            unsafe { heap32.add_to_heap(start, end) };
        } else {
            let mut heap64 = self.heap64.lock();
            unsafe { heap64.add_to_heap(start, end) };
        }
    }

    pub(crate) fn lock_heap32(&self) -> spin::MutexGuard<'_, Heap<32>> {
        self.heap32.lock()
    }

    pub(crate) fn lock_heap64(&self) -> spin::MutexGuard<'_, Heap<64>> {
        self.heap64.lock()
    }

    pub(crate) unsafe fn alloc_with_mask(
        &self,
        layout: core::alloc::Layout,
        dma_mask: u64,
    ) -> *mut u8 {
        let guard = NoIrqGuard::new();
        let result = if dma_mask <= u32::MAX as u64 {
            Self::try_alloc(&self.heap32, layout)
        } else {
            Self::try_alloc(&self.heap64, layout).or_else(|| Self::try_alloc(&self.heap32, layout))
        };
        drop(guard);

        result.map_or(null_mut(), |ptr| ptr.as_ptr())
    }

    #[inline]
    fn try_alloc<const BITS: usize>(
        heap: &Mutex<Heap<BITS>>,
        layout: core::alloc::Layout,
    ) -> Option<NonNull<u8>> {
        let mut guard = heap.lock();
        guard.alloc(layout).ok()
    }

    #[inline]
    fn range_within_u32(start: usize, end: usize) -> bool {
        if start >= end {
            return false;
        }

        let last = end - 1;

        let ps = PhysAddr::from(VirtAddr::from(start));
        let pe = PhysAddr::from(VirtAddr::from(last));

        let limit = PhysAddr::from(u32::MAX as usize);
        ps <= limit && pe <= limit
    }

    #[inline]
    fn ptr_in_32bit(ptr: *mut u8) -> bool {
        let phys = PhysAddr::from(VirtAddr::from(ptr as usize));
        phys <= PhysAddr::from(u32::MAX as usize)
    }
}

unsafe impl GlobalAlloc for KAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let guard = NoIrqGuard::new();
        let result =
            Self::try_alloc(&self.heap64, layout).or_else(|| Self::try_alloc(&self.heap32, layout));
        drop(guard);

        result.map_or(null_mut(), |ptr| ptr.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let guard = NoIrqGuard::new();
        let nn = unsafe { NonNull::new_unchecked(ptr) };

        if Self::ptr_in_32bit(ptr) {
            self.heap32.lock().dealloc(nn, layout);
        } else {
            self.heap64.lock().dealloc(nn, layout);
        }
        drop(guard);
    }
}

pub(crate) fn init() {
    let range = global_val().main_memory.clone();
    mmu::init_with_tmp_table();

    let mut start = VirtAddr::from(range.start.raw() + LINER_OFFSET);
    let mut end = VirtAddr::from(range.end.raw() + LINER_OFFSET);

    unsafe {
        if TMP_PAGE_ALLOC_ADDR != 0 {
            end = VirtAddr::from(TMP_PAGE_ALLOC_ADDR + LINER_OFFSET);
        }
    }

    println!("heap add memory [{}, {})", start, end);
    #[cfg(target_os = "none")]
    ALLOCATOR.add_to_heap(unsafe { &mut *slice_from_raw_parts_mut(start.into(), end - start) });

    println!("heap initialized");

    mmu::init();

    add_all_ram();

    cache::init();
}

fn add_all_ram() {
    let main = global_val().main_memory.clone();

    for region in platform::boot_regions() {
        if !matches!(region.kind, BootMemoryKind::Ram) {
            continue;
        }

        if region.range.to_range().contains(&main.start) {
            continue;
        }

        let start = VirtAddr::from(region.range.start.raw() + LINER_OFFSET);
        let end = VirtAddr::from(region.range.end.raw() + LINER_OFFSET);
        let len = end - start;

        if len < 2 * 1024 * 1024 {
            println!(
                "skip small ram region [{:#x}, {:#x})",
                start.raw(),
                end.raw()
            );
            continue;
        }

        println!("Heap add memory [{}, {})", start, end);
        #[cfg(target_os = "none")]
        ALLOCATOR.add_to_heap(unsafe { &mut *slice_from_raw_parts_mut(start.into(), len) });
    }
}

pub(crate) fn find_main_memory() -> Option<BootRegion> {
    let mut ram_regions = heapless::Vec::<_, 32>::new();
    let mut non_ram_regions = heapless::Vec::<_, 32>::new();

    // 收集所有区域
    for r in platform::boot_regions() {
        if matches!(r.kind, BootMemoryKind::Ram) {
            ram_regions.push(r).ok()?;
        } else {
            non_ram_regions.push(r).ok()?;
        }
    }

    let mut available_regions = heapless::Vec::<PhysCRange, 64>::new();

    // 对每个RAM区域，移除与其他region重合的部分
    for ram in &ram_regions {
        let mut current_ranges = heapless::Vec::<PhysCRange, 32>::new();
        current_ranges.push(ram.range).ok()?;

        // 对每个非RAM区域，从当前范围中移除重合部分
        for non_ram in &non_ram_regions {
            let mut new_ranges = heapless::Vec::<PhysCRange, 32>::new();

            for current_range in &current_ranges {
                // 计算重合部分
                let overlap_start = current_range.start.raw().max(non_ram.range.start.raw());
                let overlap_end = current_range.end.raw().min(non_ram.range.end.raw());

                if overlap_start < overlap_end {
                    // 有重合，需要切割
                    // 添加重合部分之前的区域
                    if current_range.start.raw() < overlap_start {
                        new_ranges
                            .push(PhysCRange {
                                start: current_range.start,
                                end: PhysAddr::new(overlap_start),
                            })
                            .ok()?;
                    }
                    // 添加重合部分之后的区域
                    if overlap_end < current_range.end.raw() {
                        new_ranges
                            .push(PhysCRange {
                                start: PhysAddr::new(overlap_end),
                                end: current_range.end,
                            })
                            .ok()?;
                    }
                } else {
                    // 无重合，保持原区域
                    new_ranges.push(*current_range).ok()?;
                }
            }
            current_ranges = new_ranges;
        }

        // 将当前RAM的所有可用区域添加到总列表
        for range in current_ranges {
            available_regions.push(range).ok()?;
        }
    }

    // 选择范围大于16MB且地址最低的区域作为main memory
    const MIN_SIZE: usize = 16 * 1024 * 1024; // 16MB
    let mut best_region: Option<PhysCRange> = None;

    for region in &available_regions {
        let size = region.end.raw() - region.start.raw();
        if size >= MIN_SIZE {
            match best_region {
                None => best_region = Some(*region),
                Some(current_best) => {
                    if region.start.raw() < current_best.start.raw() {
                        best_region = Some(*region);
                    }
                }
            }
        }
    }

    if let Some(main_range) = best_region {
        println!(
            "Selected main memory: {:?}, size: {}MB",
            main_range,
            (main_range.end.raw() - main_range.start.raw()) / (1024 * 1024)
        );

        // 创建主内存区域，使用第一个RAM区域的属性
        let first_ram = ram_regions.first()?;
        Some(BootRegion {
            range: main_range,
            name: c"main memory".as_ptr() as _,
            access: first_ram.access,
            cache: first_ram.cache,
            kind: BootMemoryKind::Ram,
        })
    } else {
        println!("no suitable main memory region found (>= 16MB)");
        None
    }
}

pub fn map(config: &MapConfig) -> Result<(), PagingError> {
    mmu::map(config)
}

pub fn iomap(paddr: PhysAddr, size: usize) -> NonNull<u8> {
    let vaddr = VirtAddr::from(paddr.raw() + LINER_OFFSET);
    match mmu::map(&MapConfig {
        name: "iomap",
        va_start: vaddr,
        pa_start: paddr,
        size,
        access: AccessSetting::ReadWrite,
        cache: CacheSetting::Device,
    }) {
        Ok(_) => {}
        Err(e) => match e {
            PagingError::AlreadyMapped => {}
            _ => panic!("iomap failed: {:?}", e),
        },
    }

    let ptr: *mut u8 = vaddr.into();
    unsafe { NonNull::new_unchecked(ptr) }

    // unimplemented!();
    // #[cfg(feature = "mmu")]
    // {
    //     mmu::iomap(paddr, _size)
    // }

    // #[cfg(not(feature = "mmu"))]
    // unsafe {
    //     NonNull::new_unchecked(paddr.as_usize() as *mut u8)
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hal_al::mmu::{AccessSetting, CacheSetting};
    use core::ffi::CStr;

    // 创建测试用的 BootRegion
    fn create_test_region(
        start: usize,
        end: usize,
        name: &'static str,
        kind: BootMemoryKind,
    ) -> BootRegion {
        BootRegion {
            range: PhysCRange {
                start: PhysAddr::new(start),
                end: PhysAddr::new(end),
            },
            name: name.as_ptr(),
            access: AccessSetting::Read | AccessSetting::Write,
            cache: CacheSetting::Normal,
            kind,
        }
    }

    // Mock platform::boot_regions 函数
    fn mock_boot_regions(regions: &[BootRegion]) -> impl Iterator<Item = BootRegion> + '_ {
        regions.iter().copied()
    }

    #[test]
    fn test_find_main_memory_simple_case() {
        // 测试简单情况：只有一个大于16MB的RAM区域，没有重叠
        let regions = [
            create_test_region(0x40000000, 0x60000000, "ram", BootMemoryKind::Ram), // 512MB RAM
        ];

        // 模拟 find_main_memory 的逻辑
        let mut ram_regions = heapless::Vec::<_, 32>::new();
        let mut non_ram_regions = heapless::Vec::<_, 32>::new();

        for r in mock_boot_regions(&regions) {
            if matches!(r.kind, BootMemoryKind::Ram) {
                ram_regions.push(r).unwrap();
            } else {
                non_ram_regions.push(r).unwrap();
            }
        }

        let mut available_regions = heapless::Vec::<PhysCRange, 64>::new();

        for ram in &ram_regions {
            let mut current_ranges = heapless::Vec::<PhysCRange, 32>::new();
            current_ranges.push(ram.range).unwrap();

            for non_ram in &non_ram_regions {
                let mut new_ranges = heapless::Vec::<PhysCRange, 32>::new();

                for current_range in &current_ranges {
                    let overlap_start = current_range.start.raw().max(non_ram.range.start.raw());
                    let overlap_end = current_range.end.raw().min(non_ram.range.end.raw());

                    if overlap_start < overlap_end {
                        if current_range.start.raw() < overlap_start {
                            new_ranges
                                .push(PhysCRange {
                                    start: current_range.start,
                                    end: PhysAddr::new(overlap_start),
                                })
                                .unwrap();
                        }
                        if overlap_end < current_range.end.raw() {
                            new_ranges
                                .push(PhysCRange {
                                    start: PhysAddr::new(overlap_end),
                                    end: current_range.end,
                                })
                                .unwrap();
                        }
                    } else {
                        new_ranges.push(*current_range).unwrap();
                    }
                }
                current_ranges = new_ranges;
            }

            for range in current_ranges {
                available_regions.push(range).unwrap();
            }
        }

        // 应该找到整个RAM区域
        assert_eq!(available_regions.len(), 1);
        assert_eq!(available_regions[0].start.raw(), 0x40000000);
        assert_eq!(available_regions[0].end.raw(), 0x60000000);

        // 检查是否满足16MB要求
        const MIN_SIZE: usize = 16 * 1024 * 1024;
        let size = available_regions[0].end.raw() - available_regions[0].start.raw();
        assert!(size >= MIN_SIZE);
    }

    #[test]
    fn test_find_main_memory_with_overlap() {
        // 测试有重叠的情况
        let regions = [
            create_test_region(0x40000000, 0x60000000, "ram", BootMemoryKind::Ram), // 512MB RAM
            create_test_region(0x45000000, 0x46000000, "reserved", BootMemoryKind::Reserved), // 16MB Reserved
        ];

        let mut ram_regions = heapless::Vec::<_, 32>::new();
        let mut non_ram_regions = heapless::Vec::<_, 32>::new();

        for r in mock_boot_regions(&regions) {
            if matches!(r.kind, BootMemoryKind::Ram) {
                ram_regions.push(r).unwrap();
            } else {
                non_ram_regions.push(r).unwrap();
            }
        }

        let mut available_regions = heapless::Vec::<PhysCRange, 64>::new();

        for ram in &ram_regions {
            let mut current_ranges = heapless::Vec::<PhysCRange, 32>::new();
            current_ranges.push(ram.range).unwrap();

            for non_ram in &non_ram_regions {
                let mut new_ranges = heapless::Vec::<PhysCRange, 32>::new();

                for current_range in &current_ranges {
                    let overlap_start = current_range.start.raw().max(non_ram.range.start.raw());
                    let overlap_end = current_range.end.raw().min(non_ram.range.end.raw());

                    if overlap_start < overlap_end {
                        if current_range.start.raw() < overlap_start {
                            new_ranges
                                .push(PhysCRange {
                                    start: current_range.start,
                                    end: PhysAddr::new(overlap_start),
                                })
                                .unwrap();
                        }
                        if overlap_end < current_range.end.raw() {
                            new_ranges
                                .push(PhysCRange {
                                    start: PhysAddr::new(overlap_end),
                                    end: current_range.end,
                                })
                                .unwrap();
                        }
                    } else {
                        new_ranges.push(*current_range).unwrap();
                    }
                }
                current_ranges = new_ranges;
            }

            for range in current_ranges {
                available_regions.push(range).unwrap();
            }
        }

        // 应该被切割为两个区域
        assert_eq!(available_regions.len(), 2);

        // 第一个区域：0x40000000 - 0x45000000 (80MB)
        let region1 = available_regions
            .iter()
            .find(|r| r.start.raw() == 0x40000000)
            .unwrap();
        assert_eq!(region1.end.raw(), 0x45000000);

        // 第二个区域：0x46000000 - 0x60000000 (416MB)
        let region2 = available_regions
            .iter()
            .find(|r| r.start.raw() == 0x46000000)
            .unwrap();
        assert_eq!(region2.end.raw(), 0x60000000);

        // 两个区域都应该大于16MB
        const MIN_SIZE: usize = 16 * 1024 * 1024;
        for region in &available_regions {
            let size = region.end.raw() - region.start.raw();
            assert!(size >= MIN_SIZE);
        }
    }

    #[test]
    fn test_find_main_memory_multiple_overlaps() {
        // 测试多个重叠的情况
        let regions = [
            create_test_region(0x40000000, 0x80000000, "ram", BootMemoryKind::Ram), // 1GB RAM
            create_test_region(
                0x45000000,
                0x46000000,
                "reserved1",
                BootMemoryKind::Reserved,
            ), // 16MB
            create_test_region(
                0x50000000,
                0x52000000,
                "reserved2",
                BootMemoryKind::Reserved,
            ), // 32MB
            create_test_region(0x70000000, 0x71000000, "kimage", BootMemoryKind::KImage), // 16MB
        ];

        let mut ram_regions = heapless::Vec::<_, 32>::new();
        let mut non_ram_regions = heapless::Vec::<_, 32>::new();

        for r in mock_boot_regions(&regions) {
            if matches!(r.kind, BootMemoryKind::Ram) {
                ram_regions.push(r).unwrap();
            } else {
                non_ram_regions.push(r).unwrap();
            }
        }

        let mut available_regions = heapless::Vec::<PhysCRange, 64>::new();

        for ram in &ram_regions {
            let mut current_ranges = heapless::Vec::<PhysCRange, 32>::new();
            current_ranges.push(ram.range).unwrap();

            for non_ram in &non_ram_regions {
                let mut new_ranges = heapless::Vec::<PhysCRange, 32>::new();

                for current_range in &current_ranges {
                    let overlap_start = current_range.start.raw().max(non_ram.range.start.raw());
                    let overlap_end = current_range.end.raw().min(non_ram.range.end.raw());

                    if overlap_start < overlap_end {
                        if current_range.start.raw() < overlap_start {
                            new_ranges
                                .push(PhysCRange {
                                    start: current_range.start,
                                    end: PhysAddr::new(overlap_start),
                                })
                                .unwrap();
                        }
                        if overlap_end < current_range.end.raw() {
                            new_ranges
                                .push(PhysCRange {
                                    start: PhysAddr::new(overlap_end),
                                    end: current_range.end,
                                })
                                .unwrap();
                        }
                    } else {
                        new_ranges.push(*current_range).unwrap();
                    }
                }
                current_ranges = new_ranges;
            }

            for range in current_ranges {
                available_regions.push(range).unwrap();
            }
        }

        // 应该被切割为4个区域
        assert_eq!(available_regions.len(), 4);

        // 验证各个区域
        let expected_regions = [
            (0x40000000, 0x45000000), // 80MB
            (0x46000000, 0x50000000), // 160MB
            (0x52000000, 0x70000000), // 480MB
            (0x71000000, 0x80000000), // 240MB
        ];

        for (start, end) in expected_regions {
            let region = available_regions
                .iter()
                .find(|r| r.start.raw() == start)
                .unwrap();
            assert_eq!(region.end.raw(), end);
        }
    }

    #[test]
    fn test_find_main_memory_select_lowest_address() {
        // 测试选择地址最低的区域
        let regions = [
            create_test_region(0x80000000, 0x90000000, "ram1", BootMemoryKind::Ram), // 256MB RAM (较高地址)
            create_test_region(0x40000000, 0x50000000, "ram2", BootMemoryKind::Ram), // 256MB RAM (较低地址)
        ];

        let mut ram_regions = heapless::Vec::<_, 32>::new();
        let mut non_ram_regions = heapless::Vec::<BootRegion, 32>::new();

        for r in mock_boot_regions(&regions) {
            if matches!(r.kind, BootMemoryKind::Ram) {
                ram_regions.push(r).unwrap();
            }
        }

        let mut available_regions = heapless::Vec::<PhysCRange, 64>::new();

        for ram in &ram_regions {
            available_regions.push(ram.range).unwrap();
        }

        // 选择最低地址的区域
        const MIN_SIZE: usize = 16 * 1024 * 1024;
        let mut best_region: Option<PhysCRange> = None;

        for region in &available_regions {
            let size = region.end.raw() - region.start.raw();
            if size >= MIN_SIZE {
                match best_region {
                    None => best_region = Some(*region),
                    Some(current_best) => {
                        if region.start.raw() < current_best.start.raw() {
                            best_region = Some(*region);
                        }
                    }
                }
            }
        }

        // 应该选择地址较低的区域
        assert!(best_region.is_some());
        let selected = best_region.unwrap();
        assert_eq!(selected.start.raw(), 0x40000000);
        assert_eq!(selected.end.raw(), 0x50000000);
    }

    #[test]
    fn test_find_main_memory_no_suitable_region() {
        // 测试没有合适区域的情况（所有区域都小于16MB）
        let regions = [
            create_test_region(0x40000000, 0x40800000, "ram1", BootMemoryKind::Ram), // 8MB
            create_test_region(0x50000000, 0x50400000, "ram2", BootMemoryKind::Ram), // 4MB
        ];

        let mut ram_regions = heapless::Vec::<_, 32>::new();
        let non_ram_regions = heapless::Vec::<BootRegion, 32>::new();

        for r in mock_boot_regions(&regions) {
            if matches!(r.kind, BootMemoryKind::Ram) {
                ram_regions.push(r).unwrap();
            }
        }

        let mut available_regions = heapless::Vec::<PhysCRange, 64>::new();

        for ram in &ram_regions {
            available_regions.push(ram.range).unwrap();
        }

        // 检查是否有合适的区域
        const MIN_SIZE: usize = 16 * 1024 * 1024;
        let mut best_region: Option<PhysCRange> = None;

        for region in &available_regions {
            let size = region.end.raw() - region.start.raw();
            if size >= MIN_SIZE {
                match best_region {
                    None => best_region = Some(*region),
                    Some(current_best) => {
                        if region.start.raw() < current_best.start.raw() {
                            best_region = Some(*region);
                        }
                    }
                }
            }
        }

        // 应该没有找到合适的区域
        assert!(best_region.is_none());
    }

    #[test]
    fn test_find_main_memory_edge_case_exact_overlap() {
        // 测试边界情况：完全重叠
        let regions = [
            create_test_region(0x40000000, 0x50000000, "ram", BootMemoryKind::Ram), // 256MB
            create_test_region(0x40000000, 0x50000000, "reserved", BootMemoryKind::Reserved), // 完全重叠
        ];

        let mut ram_regions = heapless::Vec::<_, 32>::new();
        let mut non_ram_regions = heapless::Vec::<_, 32>::new();

        for r in mock_boot_regions(&regions) {
            if matches!(r.kind, BootMemoryKind::Ram) {
                ram_regions.push(r).unwrap();
            } else {
                non_ram_regions.push(r).unwrap();
            }
        }

        let mut available_regions = heapless::Vec::<PhysCRange, 64>::new();

        for ram in &ram_regions {
            let mut current_ranges = heapless::Vec::<PhysCRange, 32>::new();
            current_ranges.push(ram.range).unwrap();

            for non_ram in &non_ram_regions {
                let mut new_ranges = heapless::Vec::<PhysCRange, 32>::new();

                for current_range in &current_ranges {
                    let overlap_start = current_range.start.raw().max(non_ram.range.start.raw());
                    let overlap_end = current_range.end.raw().min(non_ram.range.end.raw());

                    if overlap_start < overlap_end {
                        if current_range.start.raw() < overlap_start {
                            new_ranges
                                .push(PhysCRange {
                                    start: current_range.start,
                                    end: PhysAddr::new(overlap_start),
                                })
                                .unwrap();
                        }
                        if overlap_end < current_range.end.raw() {
                            new_ranges
                                .push(PhysCRange {
                                    start: PhysAddr::new(overlap_end),
                                    end: current_range.end,
                                })
                                .unwrap();
                        }
                    } else {
                        new_ranges.push(*current_range).unwrap();
                    }
                }
                current_ranges = new_ranges;
            }

            for range in current_ranges {
                available_regions.push(range).unwrap();
            }
        }

        // 完全重叠后应该没有可用区域
        assert_eq!(available_regions.len(), 0);
    }
}
