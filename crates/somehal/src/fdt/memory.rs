use heapless::Vec;

use crate::{
    kernel_code,
    mem::{MemoryDescriptor, MemoryType, page_size, virt_to_phys},
};
use kernutil::memory::merge_memories;

pub fn setup_memory_map() -> Option<()> {
    let fdt = super::fdt_base()?;

    let mut ram = Vec::<MemoryDescriptor, 32>::new();
    for memory in fdt.memory() {
        let memory = memory.ok()?;
        for region in memory.regions() {
            let region = region.ok()?;

            if ram
                .push(MemoryDescriptor {
                    physical_start: region.address as usize,
                    size_in_bytes: region.size,
                    memory_type: MemoryType::Usable,
                })
                .is_err()
            {
                println!("Warning: memory regions exceed the max supported count");
            }
        }
    }

    let mut rsv = Vec::<MemoryDescriptor, 32>::new();

    for reserved in fdt.memory_reservation_blocks() {
        if rsv
            .push(MemoryDescriptor {
                physical_start: reserved.address as usize,
                size_in_bytes: reserved.size,
                memory_type: MemoryType::Reserved,
            })
            .is_err()
        {
            println!("Warning: memory reservation blocks exceed the max supported count");
        }
    }

    for reserved in fdt.reserved_memory_regions().ok()?.flatten() {
        if let Ok(mut itr) = reserved.reg()
            && let Some(reg) = itr.next()
            && let Some(size) = reg.size
            && size > 0
            && rsv
                .push(MemoryDescriptor {
                    physical_start: reg.address as usize,
                    size_in_bytes: size,
                    memory_type: MemoryType::Reserved,
                })
                .is_err()
        {
            println!("Warning: reserved memory regions exceed the max supported count");
        }
    }

    let merged = merge_memories(&ram, &rsv, page_size());

    for desc in merged {
        crate::mem::add_memory_descriptor(desc);
    }

    Some(())
}
