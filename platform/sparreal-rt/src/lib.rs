#![no_std]
#![no_main]

extern crate somehal;

pub use sparreal_kernel::entry;
pub use sparreal_kernel::*;

mod hal_impl;

#[somehal::entry]
fn main() -> ! {
    somehal::println!("Starting Sparreal OS kernel...");
    sparreal_kernel::hal::setup::start_kernel()
}
