use core::{
    alloc::Layout,
    ffi::CStr,
    ops::Range,
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use super::{Phys, PhysAddr, PhysCRange, Virt, once::OnceStatic, region::boot_regions};
pub use arrayvec::ArrayVec;
use buddy_system_allocator::Heap;
use memory_addr::MemoryAddr;
use page_table_generic::{Access, PagingError};
use spin::MutexGuard;

pub use crate::hal_al::mmu::{AccessSetting, CacheSetting};
use crate::{
    globals::{self, cpu_inited, global_val},
    hal_al::mmu::{MapConfig, PageTableRef},
    io::print::*,
    mem::TMP_PAGE_ALLOC_ADDR,
    platform::{self, mmu::page_size},
    println,
};

#[cfg(target_os = "none")]
use crate::mem::ALLOCATOR;

pub mod table;

// pub use paging::init_table;
// pub use paging::iomap;

// pub const LINER_OFFSET: usize = 0xffff_f000_0000_0000;
pub const LINER_OFFSET: usize = 0xffff_9000_0000_0000;
static TEXT_OFFSET: OnceStatic<usize> = OnceStatic::new(0);

pub fn get_text_va_offset() -> usize {
    *TEXT_OFFSET.get_ref()
}

pub(crate) fn init_with_tmp_table() {
    println!("init tmp page table...");
    let table = new_boot_table().unwrap();
    platform::mmu::set_kernel_table(table);
}

pub(crate) fn init() {
    println!("init page table...");

    let table_ref = new_table().unwrap();
    let tb = unsafe { table::PageTable::raw_to_own(table_ref) };
    table::set_kernal_table(tb);

    unsafe {
        let start = TMP_PAGE_ALLOC_ADDR;
        let end = global_val().main_memory.end.raw();
        let len = end - start;
        let start = (start + LINER_OFFSET) as *mut u8;
        let ram = core::slice::from_raw_parts_mut(start, len);

        ALLOCATOR.add_to_heap(ram);
        println!(
            "expand heap [{:#x}, {:#x})",
            start as usize,
            start as usize + len
        );
    }
}

struct PageHeap(Heap<32>);

impl page_table_generic::Access for PageHeap {
    unsafe fn alloc(&mut self, layout: Layout) -> Option<page_table_generic::PhysAddr> {
        self.0
            .alloc(layout)
            .ok()
            .map(|ptr| (ptr.as_ptr() as usize).into())
    }

    unsafe fn dealloc(&mut self, ptr: page_table_generic::PhysAddr, layout: Layout) {
        self.0
            .dealloc(unsafe { NonNull::new_unchecked(ptr.raw() as _) }, layout);
    }

    fn phys_to_mut(&self, phys: page_table_generic::PhysAddr) -> *mut u8 {
        phys.raw() as *mut u8
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BootRegion {
    // 链接地址
    pub range: PhysCRange,
    pub name: *const u8,
    pub access: AccessSetting,
    pub cache: CacheSetting,
    pub kind: BootMemoryKind,
}

unsafe impl Send for BootRegion {}

impl BootRegion {
    pub fn new(
        range: Range<PhysAddr>,
        name: &'static CStr,
        access: AccessSetting,
        cache: CacheSetting,
        kind: BootMemoryKind,
    ) -> Self {
        Self {
            range: range.into(),
            name: name.as_ptr() as _,
            access,
            cache,
            kind,
        }
    }

    pub fn new_with_len(
        start: PhysAddr,
        len: usize,
        name: &'static CStr,
        access: AccessSetting,
        cache: CacheSetting,
        kind: BootMemoryKind,
    ) -> Self {
        Self::new(start..start + len, name, access, cache, kind)
    }

    pub fn name(&self) -> &'static str {
        unsafe { CStr::from_ptr(self.name as _).to_str().unwrap() }
    }

    // pub fn va_offset(&self) -> usize {
    //     match self.kind {
    //         RegionKind::Stack => {
    //             if cpu_inited() {
    //                 self.kind.va_offset()
    //             } else {
    //                 // cpu0
    //                 STACK_BOTTOM - self.range.start.raw()
    //             }
    //         }
    //         _ => self.kind.va_offset(),
    //     }
    // }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum BootMemoryKind {
    /// map offset as kv_offset
    KImage,
    Reserved,
    Ram,
    Mmio,
}

impl<T> From<Virt<T>> for Phys<T> {
    fn from(value: Virt<T>) -> Self {
        let v = value.raw();
        if (0xffff800000001000..0xffff900000000000).contains(&v) {
            Phys::new(v - get_text_va_offset())
        } else {
            Phys::new(v - LINER_OFFSET)
        }
    }
}
const MB: usize = 1024 * 1024;

fn new_boot_table() -> Result<PageTableRef, &'static str> {
    let mut access = PageHeap(Heap::empty());
    let main_mem = global_val().main_memory.clone();

    let tmp_end = main_mem.end;
    let tmp_size = tmp_end - main_mem.start.align_up(MB);
    let tmp_pt = (main_mem.end - tmp_size / 2).raw();

    unsafe { super::TMP_PAGE_ALLOC_ADDR = tmp_pt };

    println!("page table allocator {:#x}, {:#x}", tmp_pt, tmp_end.raw());
    unsafe { access.0.add_to_heap(tmp_pt, tmp_end.raw()) };
    new_table_with_access(&mut access)
}

fn new_table() -> Result<PageTableRef, &'static str> {
    let mut g = ALLOCATOR.lock_heap32();
    let mut access = HeapGuard(g);
    new_table_with_access(&mut access)
}

fn new_table_with_access(access: &mut dyn Access) -> Result<PageTableRef, &'static str> {
    let table = platform::mmu::new_table(access).unwrap();

    println!("map boot regions...");

    for region in platform::boot_regions() {
        let offset = match region.kind {
            BootMemoryKind::KImage => platform::mmu::kimage_va_offset(),
            _ => LINER_OFFSET,
        };

        let pa_start = region.range.start.align_down(page_size());
        let va_start: Virt<u8> = (pa_start + offset).raw().into();
        let pa_end = region.range.end.align_up(page_size());

        let size = pa_end - pa_start;
        println!(
            "  [{:<16}] [{:#x}, {:#x}) -> [{:#x}, {:#x}),\t{:?},\t{:?}",
            region.name(),
            va_start.raw(),
            va_start.raw() + size,
            pa_start.raw(),
            pa_start.raw() + size,
            region.access,
            region.cache
        );

        if let Err(e) = platform::mmu::table_map(
            table,
            access,
            &MapConfig {
                name: region.name(),
                va_start,
                pa_start,
                size,
                access: region.access,
                cache: region.cache,
            },
        ) {
            println!("map error: {e:?}");
        }
    }

    println!("Table: {table:?}");

    Ok(table)
}

struct HeapGuard<'a>(MutexGuard<'a, Heap<32>>);

impl Access for HeapGuard<'_> {
    unsafe fn alloc(&mut self, layout: Layout) -> Option<page_table_generic::PhysAddr> {
        self.0
            .alloc(layout)
            .ok()
            .map(|ptr| (ptr.as_ptr() as usize - LINER_OFFSET).into())
    }

    unsafe fn dealloc(&mut self, ptr: page_table_generic::PhysAddr, layout: Layout) {
        self.0.dealloc(
            unsafe { NonNull::new_unchecked((ptr.raw() + LINER_OFFSET) as _) },
            layout,
        );
    }

    fn phys_to_mut(&self, phys: page_table_generic::PhysAddr) -> *mut u8 {
        (phys.raw() + LINER_OFFSET) as *mut u8
    }
}

pub(crate) fn map(config: &MapConfig) -> Result<(), PagingError> {
    table::with_kernel_table(|t| t.map(config))
}
