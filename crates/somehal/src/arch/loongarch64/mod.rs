#[macro_use]
mod _macros;

mod addrspace;
mod cache;
mod context;
pub(crate) mod entry;
mod head;
mod register;
mod relocate;
mod trap;

pub use relocate::relocate;

use crate::ArchTrait;

pub struct Arch;

impl ArchTrait for Arch {
    fn kernel_code() -> &'static [u8] {
        let start = ext_sym_addr!(_head);
        let end = ext_sym_addr!(__kernel_code_end);
        unsafe { core::slice::from_raw_parts(start as *const u8, end - start) }
    }

    fn post_allocator() {}

    fn _pa(vaddr: *const u8) -> usize {
        addrspace::to_phys(vaddr as usize)
    }

    fn _va(paddr: usize) -> *mut u8 {
        addrspace::to_cache(paddr) as *mut u8
    }

    fn ioremap(paddr: usize, _size: usize) -> *mut u8 {
        Self::_io(paddr)
    }

    fn _io(paddr: usize) -> *mut u8 {
        addrspace::to_uncache(paddr) as *mut u8
    }

    fn per_cpu_trap_init(is_primary: bool) {
        trap::per_cpu_trap_init(is_primary);
    }

    fn register_timer_handler(handler: fn()) {
        trap::register_timer_handler(handler);
    }

    fn shutdown() -> ! {
        loop {
            unsafe { loongArch64::asm::idle() };
        }
    }
}
