#![no_std]

#[macro_use]
extern crate alloc;

extern crate log;

mod bar_alloc;
mod chip;
pub mod err;
mod root;
mod types;

pub use chip::PcieGeneric;
pub use rdif_pcie::Interface as Controller;
pub use rdif_pcie::{PciMem32, PciMem64, PcieController};

pub use bar_alloc::*;
pub use types::*;

pub use root::enumerate_by_controller;
