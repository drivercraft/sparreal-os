use alloc::{boxed::Box, format, string::String};

#[derive(thiserror::Error, Debug)]
pub enum DriverError {
    #[error("FDT error: {0}")]
    Fdt(String),
    #[error("Unknown driver error: {0}")]
    Unknown(String),
}

impl From<fdt_parser::FdtError<'_>> for DriverError {
    fn from(value: fdt_parser::FdtError<'_>) -> Self {
        Self::Fdt(format!("{value:?}"))
    }
}

impl From<Box<dyn core::error::Error>> for DriverError {
    fn from(value: Box<dyn core::error::Error>) -> Self {
        Self::Unknown(format!("{value:?}"))
    }
}
