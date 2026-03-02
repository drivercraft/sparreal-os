mod config;

pub use config::*;
pub use pci_types::{
    capability::PciCapability, device_type::DeviceType, Bar, CommandRegister, PciAddress,
    StatusRegister,
};

#[derive(Debug, Clone, Copy)]
pub struct BusNumber {
    pub primary: u8,
    pub secondary: u8,
    pub subordinate: u8,
}
