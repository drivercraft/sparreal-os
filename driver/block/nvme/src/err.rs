#[derive(Debug, Clone)]
pub enum Error {
    NoMemory,
    Layout,
    Dma(dma_api::DmaError),
    Unknown(&'static str),
}

pub type Result<T = ()> = core::result::Result<T, Error>;

impl From<dma_api::DmaError> for Error {
    fn from(value: dma_api::DmaError) -> Self {
        match value {
            dma_api::DmaError::NoMemory => Self::NoMemory,
            dma_api::DmaError::LayoutError(_) => Self::Layout,
            other => Self::Dma(other),
        }
    }
}
