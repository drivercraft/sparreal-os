use core::arch::naked_asm;

use crate::{entry::PrimaryCpuInitInfo, mem::phys_to_virt, smp::PerCpuMeta};

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _head() -> ! {
    naked_asm!(
        "mv t0, a0",
        "mv t1, a1",
        "lla t2, __bss_start",
        "lla t3, __bss_stop",
        "1:",
        "bgeu t2, t3, 2f",
        "sd zero, 0(t2)",
        "addi t2, t2, 8",
        "j 1b",
        "2:",
        "lla sp, __cpu0_stack_top",
        "mv a0, t0",
        "mv a1, t1",
        "j {rust_main}",
        rust_main = sym rust_main,
    )
}

fn rust_main(hart_id: usize, fdt_addr: usize) -> ! {
    super::relocate::apply();
    super::set_boot_hart_id(hart_id);
    unsafe {
        crate::fdt::FDT_ADDR = fdt_addr;
    }

    <<super::Arch as crate::ArchTrait>::Console as crate::console::ArchConsoleOps>::init();
    println!("RISC-V64 SBI kernel entry.");

    let kernel_start = super::kernel_load_address();

    crate::entry::primary_init_early(PrimaryCpuInitInfo {
        kernel_start: kernel_start.into(),
        kernel_end: (__kernel_code_end as *const () as usize).into(),
        kernel_start_link: crate::consts::VM_LOAD_ADDRESS.into(),
    });

    super::paging::enable_mmu()
}

pub(crate) fn mmu_entry() -> ! {
    super::relocate::reset();
    super::trap::setup();
    crate::prime_entry()
}

unsafe extern "C" {
    fn __kernel_code_end();
}

pub(crate) unsafe extern "C" fn _secondary_entry(arg: usize) -> ! {
    let cpu_meta = unsafe { &*(phys_to_virt(arg) as *const PerCpuMeta) };
    crate::entry::secondary_entry(cpu_meta);
    loop {
        core::hint::spin_loop();
    }
}
