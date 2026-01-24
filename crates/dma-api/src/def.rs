use core::cmp::PartialOrd;
use derive_more::{
    Add, AddAssign, Debug, Display, Div, From, Into, Mul, MulAssign, Sub, SubAssign,
};

#[derive(
    Debug,
    Display,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Hash,
    From,
    Into,
    Add,
    AddAssign,
    Mul,
    MulAssign,
    Sub,
    SubAssign,
    Div,
)]
#[debug("{}", format_args!("{_0:#X}"))]
#[display("{}", format_args!("{_0:#X}"))]
pub struct DmaAddr(u64);

impl DmaAddr {
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn checked_add(&self, rhs: u64) -> Option<Self> {
        self.0.checked_add(rhs).map(DmaAddr)
    }
}

impl PartialEq<u64> for DmaAddr {
    fn eq(&self, other: &u64) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<u64> for DmaAddr {
    fn partial_cmp(&self, other: &u64) -> Option<core::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

/// 物理地址类型
#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, Hash, From, Into, Add, Mul, Sub)]
#[debug("{}", format_args!("{_0:#X}"))]
#[display("{}", format_args!("{_0:#X}"))]
pub struct PhysAddr(u64);

impl PhysAddr {
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// DMA 传输方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DmaDirection {
    /// 数据从 CPU 传输到设备 (DMA_TO_DEVICE)
    ToDevice,
    /// 数据从设备传输到 CPU (DMA_FROM_DEVICE)
    FromDevice,
    /// 双向传输 (DMA_BIDIRECTIONAL)
    Bidirectional,
}

/// DMA 错误类型
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum DmaError {
    #[error("DMA allocation failed")]
    NoMemory,
    #[error("Invalid layout")]
    LayoutError(#[from] core::alloc::LayoutError),
    #[error("DMA address {addr} does not match device mask {mask:#X}")]
    DmaMaskNotMatch { addr: DmaAddr, mask: u64 },
    #[error("DMA align mismatch: required={required:#X}, but address={address}")]
    AlignMismatch { required: usize, address: DmaAddr },
}
