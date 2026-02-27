use core::alloc::Layout;

use kernutil::memory::MemoryType;

use crate::{
    ArchTrait, DCacheOp,
    arch::Arch,
    mem::{__kimage_va, __percpu, dcache_range, page_size, phys_to_virt, stack_size},
};

mod cpu_iter;

static mut PERCPU_START: usize = 0;
static mut PERCPU_END: usize = 0;

fn __cpu_id_list() -> impl Iterator<Item = usize> {
    cpu_iter::cpu_id_list()
}

fn align_up_pow2(value: usize, align: usize) -> usize {
    assert!(align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

fn meta_align() -> usize {
    core::mem::align_of::<PerCpuMeta>().max(64)
}

fn percpu_region_align() -> usize {
    page_size().max(meta_align())
}

fn meta_offset() -> usize {
    let link_size = percpu_link_range().len();
    let offset = align_up_pow2(link_size, meta_align());
    debug_assert_eq!(offset % meta_align(), 0);
    offset
}

/// Per-CPU data layout:
///
///
/// | Linker percpu data | align to 0x8 | PerCpuMeta | align padding to page size | Stack |
pub fn alloc_percpu() {
    println!("Initializing per-CPU data");
    let cpu_num = __cpu_id_list().count();
    let link_range = percpu_link_range();
    let link_size = link_range.len();

    let percpu_size = percpu_data_size();
    println!("Per-CPU data one cpu size: {:#x} bytes", percpu_size);
    let percpu_all_secondary_size = percpu_size * cpu_num;

    println!(
        "Total per-CPU data size for secondary CPUs: {:#x} bytes ({} CPUs)",
        percpu_all_secondary_size, cpu_num
    );

    unsafe { crate::mem::ram::flush_to_memory_map(MemoryType::Reserved) };

    let percpu_data = unsafe {
        crate::mem::ram::alloc_and_flush_to_memory_map(
            Layout::from_size_align(percpu_all_secondary_size, percpu_region_align()).unwrap(),
            MemoryType::PerCpuData,
        )
        .unwrap()
    };

    unsafe {
        PERCPU_START = percpu_data;
        PERCPU_END = PERCPU_START + percpu_all_secondary_size;

        core::ptr::write_bytes(phys_to_virt(percpu_data), 0, percpu_all_secondary_size);
    }

    println!(
        "Per-CPU data allocated at {:#x} - {:#x}",
        unsafe { PERCPU_START },
        unsafe { PERCPU_END }
    );

    let entry_virt = __kimage_va(super::entry::secondary_entry as *const () as usize);

    for (idx, hard_id) in __cpu_id_list().enumerate() {
        let cpu_percpu_start = percpu_data_range().start + idx * percpu_size;
        println!(
            "Initializing per-CPU RAM for CPU{idx} - hard id {hard_id:#x} @ {cpu_percpu_start:#x}"
        );
        unsafe {
            core::ptr::copy_nonoverlapping(
                phys_to_virt(link_range.start) as *const u8,
                phys_to_virt(cpu_percpu_start),
                link_size,
            );
        }
        let meta_start = cpu_percpu_start + meta_offset();
        let meta_va = phys_to_virt(meta_start);
        debug_assert_eq!(meta_start % meta_align(), 0);
        debug_assert_eq!((meta_va as usize) % meta_align(), 0);

        let stack_top = cpu_percpu_start + stack_offset() + stack_size();
        let stack_top_virt = __percpu(stack_top);

        let meta = PerCpuMeta {
            stack_top,
            cpu_id: hard_id,
            cpu_idx: idx,
            stack_top_virt: stack_top_virt as _,
            entry_virt: entry_virt as _,
            boot_table_paddr: 0,
        };
        unsafe {
            *meta_va.cast::<PerCpuMeta>() = meta;
        }
    }

    for meta in cpu_meta_list() {
        println!(
            "CPU{} - hard id {:#x}, stack top @{:#x}, stack top virt @{:#x}, entry virt @{:#x}",
            meta.cpu_idx, meta.cpu_id, meta.stack_top, meta.stack_top_virt, meta.entry_virt
        );
    }
}

pub(crate) fn init_percpu() {
    let boot_table = crate::mem::mmu::boot_table_addr();
    for meta in cpu_meta_list_mut() {
        meta.boot_table_paddr = boot_table;
    }

    let start = __percpu(unsafe { PERCPU_START });
    let size = unsafe { PERCPU_END - PERCPU_START };
    dcache_range(DCacheOp::CleanInvalidate, start, size);
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PerCpuMeta {
    pub stack_top: usize,
    /// The hardware ID of the CPU, e.g. hart id in RISC-V or MPIDR in ARM
    pub cpu_id: usize,
    /// The logical index of the CPU, assigned by the bootloader or determined by the OS
    pub cpu_idx: usize,

    pub stack_top_virt: usize,
    pub entry_virt: usize,

    pub boot_table_paddr: usize,
}

fn stack_offset() -> usize {
    let meta_offset = meta_offset();
    let meta_size = core::mem::size_of::<PerCpuMeta>();
    align_up_pow2(meta_offset + meta_size, page_size())
}

fn percpu_data_size() -> usize {
    align_up_pow2(stack_offset() + stack_size(), percpu_region_align())
}

#[allow(dead_code)]
/// Physical RAM allocated for per-CPU data should be mapped to this virtual address range in the kernel
pub(crate) fn percpu_range() -> core::ops::Range<usize> {
    unsafe { PERCPU_START..PERCPU_END }
}

#[allow(dead_code)]
pub(crate) fn percpu_va_range() -> core::ops::Range<usize> {
    let start = __percpu(unsafe { PERCPU_START });
    let end = __percpu(unsafe { PERCPU_END });
    start as usize..end as usize
}

pub fn cpu_meta_list() -> impl Iterator<Item = PerCpuMeta> {
    CpuMetaIter { next: 0 }
}

pub fn cpu_meta(idx: usize) -> Option<PerCpuMeta> {
    let meta_start = cpu_meta_addr(idx)?;
    let meta_va = phys_to_virt(meta_start);
    debug_assert_eq!((meta_va as usize) % meta_align(), 0);
    Some(unsafe { *(meta_va as *const PerCpuMeta) })
}

/// Physical address of cpu meta
pub(crate) fn cpu_meta_addr(idx: usize) -> Option<usize> {
    let base = percpu_data_range().start + idx * percpu_data_size();
    if base >= percpu_data_range().end {
        return None;
    }
    Some(base + meta_offset())
}

pub fn percpu_data_ptr(idx: usize) -> Option<*mut u8> {
    let base = percpu_data_range().start + idx * percpu_data_size();
    if base >= percpu_data_range().end {
        return None;
    }
    Some(phys_to_virt(base))
}

pub fn cpu_hart_id() -> usize {
    Arch::cpu_current_hartid()
}

pub fn cpu_idx() -> usize {
    let hart_id = cpu_hart_id();
    for (idx, id) in __cpu_id_list().enumerate() {
        if id == hart_id {
            return idx;
        }
    }
    panic!("Current CPU hart id {hart_id:#x} not found in CPU list");
}

pub fn cpu_id_to_idx(hart_id: usize) -> Option<usize> {
    for (idx, id) in __cpu_id_list().enumerate() {
        if id == hart_id {
            return Some(idx);
        }
    }
    None
}

pub fn cpu_idx_to_id(idx: usize) -> Option<usize> {
    __cpu_id_list().nth(idx)
}

pub fn cpu_count() -> usize {
    __cpu_id_list().count()
}

struct CpuMetaIter {
    next: usize,
}

impl Iterator for CpuMetaIter {
    type Item = PerCpuMeta;

    fn next(&mut self) -> Option<Self::Item> {
        let meta = cpu_meta(self.next)?;
        self.next += 1;
        Some(meta)
    }
}

fn cpu_meta_list_mut() -> impl Iterator<Item = &'static mut PerCpuMeta> {
    CpuMetaIterMutable { next: 0 }
}

struct CpuMetaIterMutable {
    next: usize,
}

impl Iterator for CpuMetaIterMutable {
    type Item = &'static mut PerCpuMeta;

    fn next(&mut self) -> Option<Self::Item> {
        let meta_start = cpu_meta_addr(self.next)?;
        let meta_va = phys_to_virt(meta_start);
        debug_assert_eq!((meta_va as usize) % meta_align(), 0);
        let meta = unsafe { &mut *(meta_va as *mut PerCpuMeta) };
        self.next += 1;
        Some(meta)
    }
}

fn percpu_data_range() -> core::ops::Range<usize> {
    unsafe { PERCPU_START..PERCPU_END }
}

fn percpu_link_range() -> core::ops::Range<usize> {
    unsafe extern "C" {
        fn __percpu_start();
        fn __percpu_end();
    }
    let start = __percpu_start as *const () as usize;
    let end = __percpu_end as *const () as usize;
    start..end
}
