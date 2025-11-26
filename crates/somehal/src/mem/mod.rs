use core::{cell::UnsafeCell, ops::Deref};

use byte_unit::{Byte, UnitType};
pub use kernutil::memory::{MemoryDescriptor, MemoryType};
use num_align::NumAlign;

use crate::ArchTrait;

pub(crate) mod address;
pub(crate) mod ram;
pub(crate) mod region;

static mut MMU_ENABLED: bool = false;
static MEMORY_RAM: StaticCell<heapless::Vec<MemoryDescriptor, 32>> =
    StaticCell::new(Some(heapless::Vec::new()));
static MEMORY_RSV: StaticCell<heapless::Vec<MemoryDescriptor, 32>> =
    StaticCell::new(Some(heapless::Vec::new()));

pub const MB: usize = 1024 * 1024;

pub(crate) fn set_mmu_enabled() {
    unsafe {
        MMU_ENABLED = true;
    }
}

pub(crate) fn is_mmu_enabled() -> bool {
    unsafe { MMU_ENABLED }
}

pub fn phys_to_virt(paddr: usize) -> *mut u8 {
    if is_mmu_enabled() {
        crate::arch::Arch::_va(paddr)
    } else {
        paddr as *mut u8
    }
}

pub fn virt_to_phys(vaddr: *const u8) -> usize {
    if is_mmu_enabled() {
        crate::arch::Arch::_pa(vaddr)
    } else {
        vaddr as usize
    }
}

pub fn ioremap(paddr: usize, size: usize) -> *mut u8 {
    let end = paddr + size;
    let paddr = paddr.align_down(page_size());
    let size = end.align_up(page_size()) - paddr;
    crate::arch::Arch::ioremap(paddr, size)
}

pub(crate) fn _fixmap_io(paddr: usize) -> *mut u8 {
    if is_mmu_enabled() {
        crate::arch::Arch::_io(paddr)
    } else {
        paddr as *mut u8
    }
}

pub(crate) fn early_init() {
    ram::init();
    crate::fdt::save_fdt();
}

pub(crate) fn kernel_range() -> core::ops::Range<usize> {
    let kernel = crate::arch::Arch::kernel_code().as_ptr_range();
    let start = virt_to_phys(kernel.start);
    let end = virt_to_phys(kernel.end);
    start..end
}

pub fn page_size() -> usize {
    unsafe extern "C" {
        static PAGE_SIZE: usize;
    }
    core::ptr::addr_of!(PAGE_SIZE) as usize
}

fn rsv_memories() -> heapless::Vec<MemoryDescriptor, 32> {
    let mut rsv = MEMORY_RSV.clone();
    let kernel_range = kernel_range();

    let _ = rsv.push(MemoryDescriptor {
        name: "Kernel",
        physical_start: kernel_range.start,
        size_in_bytes: kernel_range.end - kernel_range.start,
        memory_type: MemoryType::Reserved,
    });
    let _ = rsv.push(ram::to_rsvd_memory_descriptor());

    merge_contiguous_with_same_name(rsv)
}

fn merge_contiguous_with_same_name(
    mut list: heapless::Vec<MemoryDescriptor, 32>,
) -> heapless::Vec<MemoryDescriptor, 32> {
    if list.is_empty() {
        return list;
    }

    list.sort_by(|a, b| a.physical_start.cmp(&b.physical_start));

    let mut merged: heapless::Vec<MemoryDescriptor, 32> = heapless::Vec::new();
    for desc in list.into_iter() {
        if let Some(last) = merged.last_mut() {
            let last_end = last.physical_start + last.size_in_bytes;
            if last.name == desc.name && last_end == desc.physical_start {
                last.size_in_bytes += desc.size_in_bytes;
                continue;
            }
        }

        if merged.push(desc).is_err() {
            println!("Warning: reserved regions exceed the max supported count");
            break;
        }
    }

    merged
}

pub fn memory_map() -> heapless::Vec<MemoryDescriptor, 64> {
    let mut result = kernutil::memory::cal_free_memories(&MEMORY_RAM, &rsv_memories(), page_size());

    result.sort_by(|a, b| a.physical_start.cmp(&b.physical_start));

    let start = result.first().map_or(0, |m| m.physical_start);
    let end = result
        .last()
        .map_or(0, |m| m.physical_start + m.size_in_bytes);

    for rsv in rsv_memories().iter() {
        if (start..end).contains(&(rsv.physical_start)) {
            let _ = result.push(*rsv);
        }
    }

    result.sort_by(|a, b| a.physical_start.cmp(&b.physical_start));

    result
}

pub fn print_memory_map() {
    println!("Memory Map:");
    for desc in memory_map().iter() {
        let fmt = Byte::from(desc.size_in_bytes).get_appropriate_unit(UnitType::Binary);
        println!(
            "  {:<20} {:>#016x} - {:>#016x} ({:#.2})",
            desc.name,
            desc.physical_start,
            desc.physical_start + desc.size_in_bytes,
            fmt
        );
    }
}

pub(crate) fn add_memory_descriptor(desc: MemoryDescriptor) {
    if matches!(desc.memory_type, MemoryType::Usable) {
        MEMORY_RAM.update(|map| {
            if map.push(desc).is_err() {
                println!("Warning: memory usable regions exceed the max supported count");
            }
        });
    } else {
        MEMORY_RSV.update(|map| {
            if map.push(desc).is_err() {
                println!("Warning: memory reserved regions exceed the max supported count");
            }
        });
    }
}

pub(crate) struct StaticCell<T> {
    value: UnsafeCell<Option<T>>,
}

impl<T> StaticCell<T> {
    pub const fn new(v: Option<T>) -> Self {
        StaticCell {
            value: UnsafeCell::new(v),
        }
    }

    #[allow(dead_code)]
    pub fn set(&self, v: T) {
        unsafe {
            *self.value.get() = Some(v);
        }
    }

    pub fn update<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        unsafe {
            let val = &mut *self.value.get();
            f(val.as_mut().unwrap())
        }
    }

    pub fn try_deref(&self) -> Option<&T> {
        unsafe { (*self.value.get()).as_ref() }
    }
}

impl<T> Deref for StaticCell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { (*self.value.get()).as_ref().unwrap() }
    }
}

unsafe impl<T> Sync for StaticCell<T> {}
unsafe impl<T> Send for StaticCell<T> {}
