#![no_std]

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;
#[macro_use]
extern crate log;

pub mod __export;
pub mod hal;
mod lang;
mod logo;
pub mod os;

use hal::setup::start_kernel;
pub use sparreal_macros::entry;

pub fn run_kernel() -> ! {
    logo::print_logo();
    start_kernel()
}
