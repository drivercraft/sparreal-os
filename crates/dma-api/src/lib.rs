#![cfg_attr(not(any(windows, unix)), no_std)]
#![doc = include_str!("../README.md")]

extern crate alloc;

use core::{ops::Deref, ptr::NonNull};

use alloc::sync::Arc;

mod osal;

mod array;
mod common;
mod dbox;
mod slice;

pub use array::*;
pub use dbox::*;
pub use osal::Osal;
pub use slice::*;

// mod stream;

// pub use stream::*;

/// DMA 传输方向
///
/// 参考 Linux `enum dma_data_direction`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum Direction {
    /// 数据从 CPU 传输到设备 (DMA_TO_DEVICE)
    ToDevice,
    /// 数据从设备传输到 CPU (DMA_FROM_DEVICE)
    FromDevice,
    /// 双向传输 (DMA_BIDIRECTIONAL)
    Bidirectional,
}

/// DMA 地址类型
pub type DmaAddr = u64;

/// 物理地址类型
pub type PhysAddr = u64;

/// DMA 错误类型
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaError {
    #[error("DMA allocation failed")]
    NoMemory,
    #[error("Invalid layout for DMA allocation")]
    LayoutError,
    #[error("DMA address {addr:#x} does not match device mask {mask:#x}")]
    DmaMaskNotMatch { addr: DmaAddr, mask: u64 },
}

impl From<core::alloc::LayoutError> for DmaError {
    fn from(_: core::alloc::LayoutError) -> Self {
        DmaError::LayoutError
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DmaHandle {
    pub virt_addr: NonNull<u8>,
    pub dma_addr: DmaAddr,
    pub layout: core::alloc::Layout,
}

impl DmaHandle {
    pub fn new(virt_addr: NonNull<u8>, dma_addr: DmaAddr, layout: core::alloc::Layout) -> Self {
        Self {
            virt_addr,
            dma_addr,
            layout,
        }
    }
}

unsafe impl Send for DmaHandle {}

impl Deref for DmaHandle {
    type Target = core::alloc::Layout;
    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MapHandle {
    pub virt_addr: NonNull<u8>,
    pub dma_addr: DmaAddr,
    pub size: usize,
}

#[derive(Clone)]
pub struct DeviceDma {
    inner: Arc<dyn Osal>,
    mask: u64,
}

impl DeviceDma {
    pub fn new(osal: impl Osal, dma_mask: u64) -> Self {
        Self {
            inner: Arc::new(osal),
            mask: dma_mask,
        }
    }

    pub fn dma_mask(&self) -> u64 {
        self.mask
    }

    pub fn flush(&self, addr: NonNull<u8>, size: usize) {
        self.inner.flush(addr, size)
    }

    pub fn invalidate(&self, addr: NonNull<u8>, size: usize) {
        self.inner.invalidate(addr, size)
    }

    pub fn page_size(&self) -> usize {
        self.inner.page_size()
    }

    fn prepare_read(&self, ptr: NonNull<u8>, size: usize, direction: Direction) {
        self.inner.prepare_read(ptr, size, direction)
    }

    fn confirm_write(&self, ptr: NonNull<u8>, size: usize, direction: Direction) {
        self.inner.confirm_write(ptr, size, direction)
    }

    unsafe fn alloc_coherent(&self, layout: core::alloc::Layout) -> Option<DmaHandle> {
        let res = unsafe { self.inner.alloc_coherent(self.mask, layout) };
        #[cfg(debug_assertions)]
        {
            if let Some(ref handle) = res {
                assert!(
                    self.mask >= handle.dma_addr + layout.size() as u64,
                    "DMA mask not match: addr={:#x}, size={:#x}, mask={:#x}",
                    handle.dma_addr,
                    layout.size(),
                    self.mask
                );
            }
        }
        res
    }

    unsafe fn dealloc_coherent(&self, handle: DmaHandle) {
        unsafe { self.inner.dealloc_coherent(self.mask, handle) }
    }

    unsafe fn _map_single(
        &self,
        addr: NonNull<u8>,
        size: usize,
        direction: Direction,
    ) -> Result<MapHandle, DmaError> {
        let res = unsafe { self.inner.map_single(self.mask, addr, size, direction) };
        #[cfg(debug_assertions)]
        {
            if let Ok(ref handle) = res {
                assert!(
                    self.mask >= handle.dma_addr + size as u64,
                    "DMA mask not match: addr={:#x}, size={:#x}, mask={:#x}",
                    handle.dma_addr,
                    size,
                    self.mask
                );
            }
        }

        res
    }

    unsafe fn unmap_single(&self, handle: MapHandle) {
        unsafe { self.inner.unmap_single(handle) }
    }

    pub fn new_array<T>(
        &self,
        size: usize,
        align: usize,
        direction: Direction,
    ) -> Result<array::DArray<T>, DmaError> {
        array::DArray::new_zero(self, size, align, direction)
    }

    pub fn new_box<T>(
        &self,
        align: usize,
        direction: Direction,
    ) -> Result<dbox::DBox<T>, DmaError> {
        dbox::DBox::new_zero(self, align, direction)
    }

    pub fn map_single<'a, T>(
        &self,
        s: &'a [T],
        direction: Direction,
    ) -> Result<DSliceSingle<'a, T>, DmaError> {
        DSliceSingle::new(self, s, direction)
    }

    pub fn map_single_mut<'a, T>(
        &self,
        s: &'a mut [T],
        direction: Direction,
    ) -> Result<DSliceSingleMut<'a, T>, DmaError> {
        DSliceSingleMut::new(self, s, direction)
    }
}
