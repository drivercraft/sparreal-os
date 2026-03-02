#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub use core::any::Any;

#[macro_use]
mod _macros;
pub use paste::paste;
pub use rdif_def::{CpuId, KError, custom_type, irq};
pub mod io;

pub mod _rdif_prelude {
    pub use super::{CpuId, DriverGeneric, KError, io, irq::*};
}

pub trait DriverGeneric: Send + Any {
    fn open(&mut self) -> Result<(), KError>;
    fn close(&mut self) -> Result<(), KError>;

    /// Subtype casting support, returns subtype as `&dyn Any`
    fn raw_any(&self) -> Option<&dyn Any> {
        None
    }
    /// Subtype casting support, returns subtype as `&mut dyn Any`
    fn raw_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }
}
