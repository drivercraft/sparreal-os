use core::ptr::NonNull;

pub use rdif_pcie::PcieController;
use rdif_pcie::{DriverGeneric, Interface};

use crate::PciAddress;

pub struct PcieGeneric {
    mmio_base: NonNull<u8>,
}

unsafe impl Send for PcieGeneric {}

impl PcieGeneric {
    pub fn new(mmio_base: NonNull<u8>) -> Self {
        Self { mmio_base }
    }

    fn mmio_addr(&self, mmio_base: NonNull<u8>, address: PciAddress, offset: u16) -> NonNull<u32> {
        let address = (address.bus() as u32) << 20
            | (address.device() as u32) << 15
            | (address.function() as u32) << 12
            | offset as u32;
        unsafe {
            let ptr: NonNull<u32> = mmio_base.cast().add((address >> 2) as usize);
            ptr
        }
    }
}

impl DriverGeneric for PcieGeneric {
    fn open(&mut self) -> Result<(), rdif_pcie::KError> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), rdif_pcie::KError> {
        Ok(())
    }
}

impl Interface for PcieGeneric {
    fn read(&mut self, address: PciAddress, offset: u16) -> u32 {
        let ptr = self.mmio_addr(self.mmio_base, address, offset);
        unsafe { ptr.as_ptr().read_volatile() }
    }

    fn write(&mut self, address: PciAddress, offset: u16, value: u32) {
        let ptr = self.mmio_addr(self.mmio_base, address, offset);
        unsafe { ptr.as_ptr().write_volatile(value) }
    }
}
