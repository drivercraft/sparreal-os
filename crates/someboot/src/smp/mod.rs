use core::alloc::Layout;

use num_align::NumAlign;

use crate::mem::{__va, page_size, ram::Ram, stack_size};

static mut PERCPU_START: usize = 0;
static mut PERCPU_END: usize = 0;

fn __cpu_id_list() -> impl Iterator<Item = usize> {
    crate::fdt::cpu_id_list().into_iter().flatten()
}

pub fn init_percpu() -> Result<(), &'static str> {
    println!("Initializing per-CPU data");
    let cpu_num = __cpu_id_list().count();

    let percpu_size = percpu_data_size();
    println!("Per-CPU data one cpu size: {:#x} bytes", percpu_size);
    let percpu_all_secondary_size = percpu_size * cpu_num;

    println!(
        "Total per-CPU data size for secondary CPUs: {:#x} bytes ({} CPUs)",
        percpu_all_secondary_size, cpu_num
    );

    let percpu_data = Ram {}
        .alloc(Layout::from_size_align(percpu_all_secondary_size, page_size()).unwrap())
        .unwrap();

    unsafe {
        PERCPU_START = percpu_data;
        PERCPU_END = PERCPU_START + percpu_all_secondary_size;

        core::ptr::write_bytes(__va(percpu_data), 0, percpu_all_secondary_size);
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
        let meta_va = __va(meta_start);

        let meta = unsafe { &mut *meta_va.cast::<PerCpuMeta>() };
        meta.cpu_id = hard_id;
        meta.stack_top = meta_start + size_of::<PerCpuMeta>();
    }

    for meta in cpu_meta_list() {
        println!(
            "CPU{} - hard id {:#x} stack top @ {:#x}",
            meta.cpu_id, meta.cpu_id, meta.stack_top
        );
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PerCpuMeta {
    pub stack_top: usize,
    pub cpu_id: usize,
}

fn percpu_data_size() -> usize {
    (core::mem::size_of::<PerCpuMeta>() + stack_size() + percpu_link_range().len())
        .align_up(page_size())
}

pub fn cpu_meta_list() -> impl Iterator<Item = PerCpuMeta> {
    CpuMetaIter { next: 0 }
}

struct CpuMetaIter {
    next: usize,
}

impl Iterator for CpuMetaIter {
    type Item = PerCpuMeta;

    fn next(&mut self) -> Option<Self::Item> {
        let base = percpu_data_range().start + self.next * percpu_data_size();

        if self.next >= percpu_data_range().end {
            return None;
        }
        let meta = unsafe { &*(__va(base) as *const PerCpuMeta) };
        self.next += percpu_data_size();
        Some(*meta)
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
