#![no_std]

extern crate alloc;

mod command;
pub mod err;
mod nvme;
mod queue;
mod registers;

use core::{alloc::Layout, ptr::NonNull};

pub use nvme::{Config, Namespace, Nvme};

#[derive(Clone, Copy)]
pub struct DMAMem {
    pub virt: NonNull<u8>,
    pub phys: u64,
    pub layout: Layout,
}
