use core::ops::Range;

use heapless::Vec;

use crate::{
    consts::PAGE_SIZE,
    fdt::fdt_base,
    mem::{MemoryDescriptor, MemoryType, add_memory_descriptor},
};

pub fn init_memory_map() -> Option<()> {
    let fdt = super::fdt_base()?;
    let mut reserved_descs = Vec::<MemoryDescriptor, 128>::new();

    for memory in fdt.memory() {
        for region in memory.regions() {
            if region.size == 0 {
                continue;
            }

            add_memory_descriptor(MemoryDescriptor {
                physical_start: region.address as usize,
                size_in_bytes: region.size as _,
                memory_type: MemoryType::Free,
            })
            .unwrap();
        }
    }

    for reserved in fdt.memory_reservations() {
        reserved_descs
            .push(MemoryDescriptor::new_aligned(
                reserved.address as usize,
                reserved.size as usize,
                MemoryType::Reserved,
                PAGE_SIZE,
            ))
            .ok()?;
    }

    for reserved in fdt.reserved_memory() {
        if let Some(regs) = reserved.reg() {
            for reg in regs {
                if let Some(size) = reg.size
                    && size > 0
                {
                    reserved_descs
                        .push(MemoryDescriptor::new_aligned(
                            reg.address as usize,
                            size as usize,
                            MemoryType::Reserved,
                            PAGE_SIZE,
                        ))
                        .ok()?;
                }
            }
        }
    }

    merge_reserved_descriptors(&mut reserved_descs);

    for reserved in reserved_descs {
        add_memory_descriptor(reserved).unwrap();
    }

    Some(())
}

fn merge_reserved_descriptors(descs: &mut Vec<MemoryDescriptor, 128>) {
    descs.sort_unstable_by_key(|desc| desc.physical_start);

    let mut merged = Vec::<MemoryDescriptor, 128>::new();
    for desc in descs.iter().cloned() {
        if let Some(last) = merged.last_mut() {
            let last_end = last.physical_start + last.size_in_bytes;
            let desc_end = desc.physical_start + desc.size_in_bytes;
            if desc.physical_start <= last_end {
                last.size_in_bytes = desc_end.max(last_end) - last.physical_start;
                continue;
            }
        }

        merged
            .push(desc)
            .expect("reserved descriptor capacity exceeded");
    }

    descs.clear();
    for desc in merged {
        descs
            .push(desc)
            .expect("reserved descriptor capacity exceeded");
    }
}

pub fn memories() -> impl Iterator<Item = Range<usize>> {
    let mut res = Vec::<_, 128>::new();
    if let Some(fdt) = fdt_base() {
        for memory in fdt.memory() {
            for region in memory.regions() {
                res.push(region.address as usize..(region.address + region.size) as usize)
                    .ok();
            }
        }
    }
    res.into_iter()
}
