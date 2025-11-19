use heapless::Vec;
use kernutil::memory::merge_memories;
use uefi_raw::table::boot::{MemoryDescriptor, MemoryType};

use crate::mem::page_size;

pub fn setup_memory_map<'a>(
    mems: impl Iterator<Item = &'a MemoryDescriptor>,
) -> anyhow::Result<()> {
    let mut ram = Vec::<crate::mem::MemoryDescriptor, 128>::new();
    let mut rsv = Vec::<crate::mem::MemoryDescriptor, 128>::new();
    for memory in mems {
        match memory.ty {
            MemoryType::CONVENTIONAL
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA => {
                if ram
                    .push(crate::mem::MemoryDescriptor {
                        physical_start: memory.phys_start as _,
                        size_in_bytes: memory.page_count as usize * page_size(),
                        memory_type: crate::mem::MemoryType::Usable,
                    })
                    .is_err()
                {
                    println!("Warning: memory regions exceed the max supported count");
                }
            }
            _ => {
                if rsv
                    .push(crate::mem::MemoryDescriptor {
                        physical_start: memory.phys_start as _,
                        size_in_bytes: memory.page_count as usize * page_size(),
                        memory_type: crate::mem::MemoryType::Reserved,
                    })
                    .is_err()
                {
                    println!("Warning: memory regions exceed the max supported count");
                }
            }
        }
    }

    let _ = rsv.push(crate::mem::ram::to_rsvd_memory_descriptor());

    let merged = merge_memories(&ram, &rsv, page_size());

    for desc in merged {
        crate::mem::add_memory_descriptor(desc);
    }

    Ok(())
}

fn memty_str(t: &MemoryType) -> &'static str {
    match *t {
        MemoryType::RESERVED => "RESERVED",
        MemoryType::LOADER_CODE => "LOADER_CODE",
        MemoryType::LOADER_DATA => "LOADER_DATA",
        MemoryType::BOOT_SERVICES_CODE => "BOOT_SERVICES_CODE",
        MemoryType::BOOT_SERVICES_DATA => "BOOT_SERVICES_DATA",
        MemoryType::RUNTIME_SERVICES_CODE => "RUNTIME_SERVICES_CODE",
        MemoryType::RUNTIME_SERVICES_DATA => "RUNTIME_SERVICES_DATA",
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
