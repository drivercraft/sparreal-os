use uefi::boot::{MemoryDescriptor, MemoryType};

use crate::mem::{add_memory_descriptors, page_size};

pub fn setup_memory_map<'a>(
    mems: impl Iterator<Item = &'a MemoryDescriptor>,
) -> anyhow::Result<()> {
    add_memory_descriptors(mems.map(|memory| match memory.ty {
        MemoryType::CONVENTIONAL
        | MemoryType::BOOT_SERVICES_CODE
        | MemoryType::BOOT_SERVICES_DATA
        | MemoryType::LOADER_CODE
        | MemoryType::LOADER_DATA => crate::mem::MemoryDescriptor {
            name: "RAM",
            physical_start: memory.phys_start as _,
            size_in_bytes: memory.page_count as usize * page_size(),
            memory_type: crate::mem::MemoryType::Free,
        },
        MemoryType::MMIO | MemoryType::MMIO_PORT_SPACE => crate::mem::MemoryDescriptor {
            name: memty_str(&memory.ty),
            physical_start: memory.phys_start as _,
            size_in_bytes: memory.page_count as usize * page_size(),
            memory_type: crate::mem::MemoryType::Mmio,
        },
        t => crate::mem::MemoryDescriptor {
            name: memty_str(&t),
            physical_start: memory.phys_start as _,
            size_in_bytes: memory.page_count as usize * page_size(),
            memory_type: crate::mem::MemoryType::Reserved,
        },
    }))?;

    Ok(())
}

fn memty_str(t: &MemoryType) -> &'static str {
    match *t {
        MemoryType::RESERVED => "RESERVED",
        MemoryType::LOADER_CODE => "LOADER_CODE",
        MemoryType::LOADER_DATA => "LOADER_DATA",
        MemoryType::BOOT_SERVICES_CODE => "BOOT_SERVICES_CODE",
        MemoryType::BOOT_SERVICES_DATA => "BOOT_SERVICES_DATA",
        MemoryType::RUNTIME_SERVICES_CODE => "UEFI Runtime",
        MemoryType::RUNTIME_SERVICES_DATA => "UEFI Runtime",
        MemoryType::CONVENTIONAL => "CONVENTIONAL",
        MemoryType::UNUSABLE => "UNUSABLE",
        MemoryType::PAL_CODE => "PAL_CODE",
        MemoryType::MMIO => "MMIO",
        MemoryType::MMIO_PORT_SPACE => "MMIO_PORT_SPACE",
        MemoryType::ACPI_NON_VOLATILE => "ACPI_NON_VOLATILE",
        MemoryType::ACPI_RECLAIM => "ACPI_RECLAIM",
        MemoryType::UNACCEPTED => "UNACCEPTED",
        _ => "UNKNOWN",
    }
}
