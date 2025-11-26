#![no_std]
#![no_main]
#![feature(iter_next_chunk)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate core;

#[macro_use]
extern crate log;

#[macro_use]
pub mod console;

#[cfg(target_arch = "loongarch64")]
#[path = "arch/loongarch64/mod.rs"]
pub mod arch;

#[cfg(target_arch = "aarch64")]
#[path = "arch/aarch64/mod.rs"]
pub mod arch;

mod acpi;
mod cmdline;
mod consts;
#[cfg(efi)]
mod efi_stub;
mod elf;
pub(crate) mod fdt;
pub mod mem;
pub mod irq;
pub mod power;

pub use somehal_macros::{entry, secondary_entry};

trait ArchTrait {
    fn kernel_code() -> &'static [u8];
    fn post_allocator();

    fn per_cpu_trap_init(is_primary: bool);

    fn _pa(vaddr: *const u8) -> usize;
    fn _va(paddr: usize) -> *mut u8;
    fn _io(paddr: usize) -> *mut u8;
    fn ioremap(paddr: usize, size: usize) -> *mut u8;

    fn register_timer_handler(handler: fn());
    fn shutdown() -> !;
        
}

pub fn post_allocator() {
    debug!("Setup after allocator");
    arch::Arch::post_allocator();
}

fn kernel_code() -> &'static [u8] {
    arch::Arch::kernel_code()
}

fn prime_entry() -> ! {
    mem::set_mmu_enabled();
    arch::Arch::per_cpu_trap_init(true);
    fdt::setup_earlycon();
    fdt::setup_memory_map();
    let _ = acpi::earlycon::acpi_setup_earlycon();

    mem::print_memory_map();

    unsafe extern "C" {
        fn __somehal_main() -> !;
    }
    unsafe { __somehal_main() }
}
