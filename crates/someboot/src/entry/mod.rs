use page_table_generic::{PhysAddr, VirtAddr};

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
