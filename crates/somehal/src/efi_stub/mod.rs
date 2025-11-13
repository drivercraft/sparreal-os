use core::{fmt::Write, hint::spin_loop, ptr::null, sync::atomic::AtomicBool};

use uefi::{
    Result,
    boot::{MemoryDescriptor, MemoryType},
    mem::memory_map::MemoryMap,
    prelude::*,
    proto::loaded_image::LoadedImage,
    system::with_config_table,
    table::cfg::ConfigTableEntry,
};
use uefi_raw::table::system::SystemTable;

use crate::{
    acpi::set_rsdp,
    arch::relocate,
    mem::{self, MB, page_size},
};

mod acpi_handle;
mod earlycon;
pub mod pe;

pub(crate) use earlycon::acpi_setup_earlycon;

/// EFI PE 入口点 - 符合 EFI ABI 的汇编包装
/// 参数: a0 = image_handle, a1 = system_table
#[unsafe(export_name = "efi_pe_entry")]
#[unsafe(link_section = ".text")]
pub unsafe extern "C" fn efi_pe_entry(
    image_handle: Handle,
    system_table: *const SystemTable,
) -> Status {
    unsafe {
        relocate();
        ::uefi::boot::set_image_handle(image_handle);
        ::uefi::table::set_system_table(system_table);

        crate::console::set_out(&UefiPrinter);

        if let Err(e) = efi_main() {
            println!("EFI application error: {:?}", e);
            return e.status();
        }

        crate::arch::entry::efi_setup();

        UEFI_SERVICE_OK.store(false, core::sync::atomic::Ordering::Relaxed);
        let mem_map = boot::exit_boot_services(None);

        println!("Exited boot services, owned memory map obtained.");

        for desc in mem_map.entries() {
            if matches!(desc.ty, MemoryType::CONVENTIONAL)
                && desc.page_count as usize >= 2 * MB / page_size()
            {
                println!("{desc:#x?}");
                let desc: crate::mem::region::MemoryDescriptor = desc.into();
                mem::add_memory_descriptor(desc.into());
            }
        }

        crate::arch::entry::kernel_entry(1, null(), system_table as *const core::ffi::c_void);
    }
}

fn efi_main() -> Result {
    find_acpi_rsdp();

    println!("Page size: {:#x} bytes", crate::mem::page_size());

    let h = boot::get_handle_for_protocol::<LoadedImage>()?;

    let img = boot::open_protocol_exclusive::<LoadedImage>(h)?;

    match img.load_options_as_cstr16() {
        Ok(cmdline) => {
            println!("Kernel command line: {}", cmdline);
            system::with_stdout(|stdout| {
                let _ = cmdline.as_str_in_buf(stdout);
            });
        }
        Err(e) => {
            println!("Failed to get load options as CStr16: {:?}", e);
        }
    }

    Ok(())
}

#[unsafe(link_section = ".data")]
static UEFI_SERVICE_OK: AtomicBool = AtomicBool::new(true);

struct UefiPrinter;
impl crate::console::Con for UefiPrinter {
    fn write_str(&self, s: &str) {
        if !UEFI_SERVICE_OK.load(core::sync::atomic::Ordering::Relaxed) {
            return;
        }
        system::with_stdout(|stdout| {
            let _ = stdout.write_str(s);
        });
    }
}

fn find_acpi_rsdp() {
    with_config_table(|config_table| {
        let mut version = 0;
        let mut addr = null();

        for entry in config_table {
            if entry.guid == ConfigTableEntry::ACPI2_GUID {
                // ACPI 2.0 RSDP (推荐)
                println!("Found ACPI 2.0 RSDP at address: {:p}", entry.address);
                version = 2;
                addr = entry.address;
                break;
            }

            if entry.guid == ConfigTableEntry::ACPI_GUID {
                // ACPI 1.0 RSDP (备选)
                println!("Found ACPI 1.0 RSDP at address: {:p}", entry.address);
                if version == 0 {
                    version = 1;
                    addr = entry.address;
                }
            }
        }

        if !addr.is_null() {
            println!("Using ACPI {} RSDP at address: {:p}", version, addr);
            set_rsdp(addr);
        } else {
            println!("No ACPI RSDP found in UEFI config tables.");
        }
    })
}

impl From<&MemoryDescriptor> for crate::mem::region::MemoryDescriptor {
    fn from(value: &MemoryDescriptor) -> Self {
        let name = match value.ty {
            MemoryType::CONVENTIONAL => "Conventional Memory",
            MemoryType::LOADER_CODE => "Loader Code",
            MemoryType::LOADER_DATA => "Loader Data",
            MemoryType::BOOT_SERVICES_CODE => "Boot Services Code",
            MemoryType::BOOT_SERVICES_DATA => "Boot Services Data",
            MemoryType::RUNTIME_SERVICES_CODE => "Runtime Services Code",
            MemoryType::RUNTIME_SERVICES_DATA => "Runtime Services Data",
            MemoryType::MMIO => "MMIO",
            MemoryType::MMIO_PORT_SPACE => "MMIO Port Space",
            MemoryType::PAL_CODE => "PAL Code",
            _ => "Reserved",
        };

        crate::mem::region::MemoryDescriptor {
            name,
            physical_start: value.phys_start as usize,
            size_in_bytes: (value.page_count as usize) * 0x1000,
            memory_type: match value.ty {
                MemoryType::CONVENTIONAL => crate::mem::region::MemoryType::Usable,
                _ => crate::mem::region::MemoryType::Reserved,
            },
        }
    }
}

impl From<crate::mem::region::MemoryDescriptor> for kernutil::memory::MemoryDescriptor {
    fn from(value: crate::mem::region::MemoryDescriptor) -> Self {
        kernutil::memory::MemoryDescriptor {
            physical_start: value.physical_start,
            size_in_bytes: value.size_in_bytes,
            memory_type: match value.memory_type {
                crate::mem::region::MemoryType::Usable => kernutil::memory::MemoryType::Usable,
                crate::mem::region::MemoryType::Reserved => kernutil::memory::MemoryType::Reserved,
            },
        }
    }
}
