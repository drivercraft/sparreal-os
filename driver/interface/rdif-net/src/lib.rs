#![no_std]

pub use rdif_base::{DriverGeneric, KError, io};

/// Operations that require a block storage device driver to implement.
pub trait Interface: DriverGeneric {}
