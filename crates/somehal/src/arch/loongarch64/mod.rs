#[macro_use]
mod _macros;

mod addrspace;
pub(crate) mod entry;
mod head;
mod register;
mod relocate;

pub use relocate::relocate;

use crate::ArchTrait;

static mut IS_MMU_ENABLED: bool = false;

fn is_mmu_enabled() -> bool {
    unsafe { IS_MMU_ENABLED }
}

pub struct Arch;

impl ArchTrait for Arch {
    fn kernel_code() -> &'static [u8] {
        let start = ext_sym_addr!(_head);
        let end = ext_sym_addr!(__kernel_code_end);
        unsafe { core::slice::from_raw_parts(start as *const u8, end - start) }
    }

    fn post_allocator() {}

    fn _pa(vaddr: *mut u8) -> usize {
        addrspace::to_phys(vaddr as usize)
    }

    fn _va(paddr: usize) -> *mut u8 {
        addrspace::to_cache(paddr) as *mut u8
    }

    fn ioremap(paddr: usize, _size: usize) -> *mut u8 {
        if is_mmu_enabled() {
            addrspace::to_uncache(paddr) as *mut u8
        } else {
            paddr as *mut u8
        }
    }
}
