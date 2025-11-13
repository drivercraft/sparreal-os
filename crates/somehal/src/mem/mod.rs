use core::{cell::UnsafeCell, ops::Deref};

pub use kernutil::memory::{MemoryDescriptor, MemoryType};
use num_align::NumAlign;

use crate::ArchTrait;

pub(crate) mod address;
pub(crate) mod ram;
pub(crate) mod region;

static mut MMU_ENABLED: bool = false;
static MEMORY_MAP: StaticCell<heapless::Vec<MemoryDescriptor, 64>> =
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
    crate::arch::Arch::_fixmap_io(paddr)
}

pub(crate) fn early_init() {
    ram::init();
    crate::fdt::save_fdt();
}

pub(crate) fn kernel_range() -> core::ops::Range<usize> {
    let kernel = crate::arch::Arch::kernel_code().as_ptr_range();
    let start = kernel.start as usize;
    let end = ram::current() as usize;
    start..end
}

pub fn page_size() -> usize {
    unsafe extern "C" {
        static PAGE_SIZE: usize;
    }
    core::ptr::addr_of!(PAGE_SIZE) as usize
}

pub(crate) fn add_memory_descriptor(desc: MemoryDescriptor) {
    MEMORY_MAP.update(|map| {
        let _ = map.push(desc);
    });
}

pub fn get_memory_map() -> &'static [MemoryDescriptor] {
    &MEMORY_MAP
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
}

impl<T> Deref for StaticCell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { (*self.value.get()).as_ref().unwrap() }
    }
}

unsafe impl<T> Sync for StaticCell<T> {}
unsafe impl<T> Send for StaticCell<T> {}
