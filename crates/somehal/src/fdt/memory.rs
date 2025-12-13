use crate::{
    consts::PAGE_SIZE,
    mem::{MemoryDescriptor, MemoryType, add_memory_descriptor},
};

pub fn setup_memory_map() -> Option<()> {
    let fdt = super::fdt_base()?;

    for memory in fdt.memory() {
        let memory = memory.ok()?;
        for region in memory.regions() {
            let region = region.ok()?;

            add_memory_descriptor(MemoryDescriptor {
                name: "Ram",
                physical_start: region.address as usize,
                size_in_bytes: region.size,
                memory_type: MemoryType::Free,
            })
            .unwrap();
        }
    }

    for reserved in fdt.memory_reservation_blocks() {
        add_memory_descriptor(MemoryDescriptor::new_aligned(
            "FDT Reserved",
            reserved.address as usize,
            reserved.size,
            MemoryType::Reserved,
            PAGE_SIZE,
        ))
        .unwrap();
    }

    // for reserved in fdt.reserved_memory_regions().ok()?.flatten() {
    //     if let Ok(mut itr) = reserved.reg()
    //         && let Some(reg) = itr.next()
    //         && let Some(size) = reg.size
    //         && size > 0
    //     {
    //         add_memory_descriptor(MemoryDescriptor {
    //             name: reserved.name(),
    //             physical_start: reg.address as usize,
    //             size_in_bytes: size,
    //             memory_type: MemoryType::Reserved,
    //         });
    //     }
    // }

    Some(())
}
