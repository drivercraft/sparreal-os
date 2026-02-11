use core::alloc::Layout;

use anyhow::anyhow;
use num_align::NumAlign;

use crate::mem::{__va, page_size, phys_to_virt, ram::Ram, stack_size};

static mut PERCPU_START: usize = 0;
static mut PERCPU_END: usize = 0;

static mut CPU_ID_LIST_START: usize = 0;
static mut CPU_ID_LIST_END: usize = 0;

fn __cpu_id_list() -> impl Iterator<Item = usize> {
    crate::fdt::cpu_id_list().into_iter().flatten()
}

pub fn init_percpu() -> anyhow::Result<()> {
    println!("Initializing per-CPU data");
    init_cpu_id_list()?;

    let percpu_size = (percpu_link_range().len() + stack_size()).align_up(page_size());
    println!("Per-CPU data one cpu size: {:#x} bytes", percpu_size);

    let percpu_all_secondary_size = percpu_size * (__cpu_id_list().count() - 1);

    let percpu_data = Ram {}
        .alloc(Layout::from_size_align(percpu_all_secondary_size, page_size()).unwrap())
        .ok_or(anyhow!("Ram no memory"))?;

    unsafe {
        PERCPU_START = percpu_data;
        PERCPU_END = PERCPU_START + percpu_size;
    }

    for cpu_id in __cpu_id_list() {
        println!("Initializing per-CPU RAM for CPU {}", cpu_id);
    }

    Ok(())
}

fn init_cpu_id_list() -> anyhow::Result<()> {
    let cpu_num = __cpu_id_list().count();
    let layout = Layout::array::<usize>(cpu_num).map_err(|_| anyhow!("CPU ID list too large"))?;
    println!("CPU num: {}", cpu_num);
    let cpu_id_list_ptr = Ram {}.alloc(layout).ok_or(anyhow!("Ram no memory"))?;

    unsafe {
        CPU_ID_LIST_START = cpu_id_list_ptr;
        CPU_ID_LIST_END = CPU_ID_LIST_START + cpu_num * core::mem::size_of::<usize>();

        let mut ptr = __va(CPU_ID_LIST_START) as *mut usize;
        for cpu_id in __cpu_id_list() {
            println!("CPU ID: {}", cpu_id);
            ptr.write(cpu_id);
            ptr = ptr.add(1);
        }
    }

    Ok(())
}

pub fn cpu_hard_id_list() -> &'static [usize] {
    unsafe {
        let start = CPU_ID_LIST_START as *const usize;
        let end = CPU_ID_LIST_END as *const usize;
        core::slice::from_raw_parts(
            start,
            (end as usize - start as usize) / core::mem::size_of::<usize>(),
        )
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
