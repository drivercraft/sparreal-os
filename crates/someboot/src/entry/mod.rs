use page_table_generic::{PhysAddr, VirtAddr};

use crate::smp::PerCpuMeta;

pub struct PrimaryCpuInitInfo {
    pub kernel_start: PhysAddr,
    pub kernel_end: PhysAddr,
    pub kernel_start_link: VirtAddr,
}

pub fn primary_init_early(params: PrimaryCpuInitInfo) {
    crate::mem::setup_entry(
        params.kernel_start,
        params.kernel_end,
        params.kernel_start_link,
    );

    crate::fdt::setup_earlycon();
    let _ = crate::acpi::earlycon::acpi_setup_earlycon();

    #[cfg(efi)]
    crate::efi_stub::exit_boot_services();

    if let Some(cmdline) = crate::cmdline::cmdline() {
        println!("{cmdline}");
    }
    println!("VM Load @{:#x}", params.kernel_start);
    println!("VM Load Offset: {:#x}", crate::mem::vm_load_offset());

    crate::mem::early_init();
}

pub(crate) fn secondary_entry(_cpu_meta: &PerCpuMeta) {
    unsafe extern "Rust" {
        fn __someboot_secondary();
    }
    unsafe { __someboot_secondary() };
    // println!("Secondary CPU {} (ID {}) is starting up", cpu_meta.cpu_idx, cpu_meta.cpu_id);
    // println!("Stack top: {:#x}", cpu_meta.stack_top);
    // println!("MMU entry: {:#x}", cpu_meta.entry_virt);
}
