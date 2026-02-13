use core::alloc::Layout;

use kernutil::memory::MemoryType;
use num_align::NumAlign;

use crate::mem::{page_size, phys_to_virt, stack_size};

mod cpu_iter;

static mut PERCPU_START: usize = 0;
static mut PERCPU_END: usize = 0;

fn __cpu_id_list() -> impl Iterator<Item = usize> {
    cpu_iter::cpu_id_list()
}

/// Per-CPU data layout:
///
///
/// | Linker percpu data | PerCpuMeta | align padding to page size | Stack |
pub fn init_percpu() {
    println!("Initializing per-CPU data");
    let cpu_num = __cpu_id_list().count();

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
            Layout::from_size_align(percpu_all_secondary_size, page_size()).unwrap(),
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

    for (idx, hard_id) in __cpu_id_list().enumerate() {
        let cpu_percpu_start = percpu_data_range().start + idx * percpu_size;
        println!(
            "Initializing per-CPU RAM for CPU{idx} - hard id {hard_id:#x} @ {cpu_percpu_start:#x}"
        );
        let meta_start = cpu_percpu_start + percpu_link_range().len();
        let meta_va = phys_to_virt(meta_start);

        let meta = unsafe { &mut *meta_va.cast::<PerCpuMeta>() };
        meta.cpu_id = hard_id;
        meta.stack_top = cpu_percpu_start + stack_offset() + stack_size();
    }

    for meta in cpu_meta_list() {
        println!(
            "CPU{} - hard id {:#x} stack top @ {:#x}",
            meta.cpu_id, meta.cpu_id, meta.stack_top
        );
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PerCpuMeta {
    pub stack_top: usize,
    pub cpu_id: usize,
}

fn stack_offset() -> usize {
    let link_size = percpu_link_range().len();
    let meta_size = core::mem::size_of::<PerCpuMeta>();
    (link_size + meta_size).align_up(page_size())
}

fn percpu_data_size() -> usize {
    (stack_offset() + stack_size()).align_up(page_size())
}

#[allow(dead_code)]
/// Physical RAM allocated for per-CPU data should be mapped to this virtual address range in the kernel
pub(crate) fn percpu_range() -> core::ops::Range<usize> {
    unsafe { PERCPU_START..PERCPU_END }
}

pub fn cpu_meta_list() -> impl Iterator<Item = PerCpuMeta> {
    CpuMetaIter { next: 0 }
}

pub fn cpu_meta(idx: usize) -> Option<PerCpuMeta> {
    let base = percpu_data_range().start + idx * percpu_data_size();
    if base >= percpu_data_range().end {
        return None;
    }

    let meta_start = base + percpu_link_range().len();
    Some(unsafe { *(phys_to_virt(meta_start) as *const PerCpuMeta) })
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
