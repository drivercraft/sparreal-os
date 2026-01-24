#![cfg_attr(target_os = "none", no_std)]
#![doc = include_str!("../README.md")]

extern crate alloc;

use core::{num::NonZeroUsize, ops::Deref, ptr::NonNull};

mod osal;

mod array;
mod common;
mod dbox;
mod def;
mod map_single;

pub use array::*;
pub use dbox::*;
pub use def::*;
pub use map_single::*;
pub use osal::DmaOp;

impl Deref for DmaHandle {
    type Target = core::alloc::Layout;
    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

#[derive(Clone)]
pub struct DeviceDma {
    os: &'static dyn DmaOp,
    mask: u64,
}

impl DeviceDma {
    pub fn new(dma_mask: u64, osal: &'static dyn DmaOp) -> Self {
        Self {
            mask: dma_mask,
            os: osal,
        }
    }

    pub fn dma_mask(&self) -> u64 {
        self.mask
    }

    pub fn flush(&self, addr: NonNull<u8>, size: usize) {
        self.os.flush(addr, size)
    }

    pub fn invalidate(&self, addr: NonNull<u8>, size: usize) {
        self.os.invalidate(addr, size)
    }

    pub fn flush_invalidate(&self, addr: NonNull<u8>, size: usize) {
        self.os.flush_invalidate(addr, size)
    }

    pub fn page_size(&self) -> usize {
        self.os.page_size()
    }

    fn prepare_read(
        &self,
        handle: &DmaMapHandle,
        offset: usize,
        size: usize,
        direction: DmaDirection,
    ) {
        self.os.prepare_read(handle, offset, size, direction)
    }

    fn confirm_write(
        &self,
        handle: &DmaMapHandle,
        offset: usize,
        size: usize,
        direction: DmaDirection,
    ) {
        self.os.confirm_write(handle, offset, size, direction)
    }

    unsafe fn alloc_coherent(&self, layout: core::alloc::Layout) -> Result<DmaHandle, DmaError> {
        let res = unsafe { self.os.alloc_coherent(self.mask, layout) }.ok_or(DmaError::NoMemory)?;
        match self.check_handle(&res) {
            Ok(()) => (),
            Err(e) => {
                unsafe {
                    self.dealloc_coherent(res);
                }
                return Err(e);
            }
        }
        Ok(res)
    }

    unsafe fn dealloc_coherent(&self, handle: DmaHandle) {
        unsafe { self.os.dealloc_coherent(handle) }
    }

    fn check_handle(&self, handle: &DmaHandle) -> Result<(), DmaError> {
        let addr: u64 = handle.dma_addr.into();

        let in_mask = if handle.size() == 0 {
            addr <= self.dma_mask()
        } else {
            addr.checked_add(handle.size().saturating_sub(1) as u64)
                .map(|end| end <= self.dma_mask())
                .unwrap_or(false)
        };

        if !in_mask {
            return Err(DmaError::DmaMaskNotMatch {
                addr: handle.dma_addr,
                mask: self.dma_mask(),
            });
        }

        let is_aligned = handle
            .dma_addr
            .as_u64()
            .is_multiple_of(handle.align() as u64);
        if !is_aligned {
            return Err(DmaError::AlignMismatch {
                address: handle.dma_addr,
                required: handle.align(),
            });
        }

        Ok(())
    }

    unsafe fn _map_single(
        &self,
        addr: NonNull<u8>,
        size: NonZeroUsize,
        align: usize,
        direction: DmaDirection,
    ) -> Result<DmaMapHandle, DmaError> {
        let res = unsafe { self.os.map_single(self.mask, addr, size, align, direction) }?;
        match self.check_handle(&res) {
            Ok(()) => (),
            Err(e) => {
                unsafe {
                    self.unmap_single(res);
                }
                return Err(e);
            }
        }
        Ok(res)
    }

    unsafe fn unmap_single(&self, handle: DmaMapHandle) {
        unsafe { self.os.unmap_single(handle) }
    }

    pub fn array_zero<T>(
        &self,
        size: usize,
        direction: DmaDirection,
    ) -> Result<array::DArray<T>, DmaError> {
        array::DArray::new_zero(self, size, direction)
    }

    pub fn array_zero_with_align<T>(
        &self,
        size: usize,
        align: usize,
        direction: DmaDirection,
    ) -> Result<array::DArray<T>, DmaError> {
        array::DArray::new_zero_with_align(self, size, align, direction)
    }

    pub fn box_zero<T>(&self, direction: DmaDirection) -> Result<dbox::DBox<T>, DmaError> {
        dbox::DBox::new_zero(self, direction)
    }

    pub fn box_zero_with_align<T>(
        &self,
        align: usize,
        direction: DmaDirection,
    ) -> Result<dbox::DBox<T>, DmaError> {
        dbox::DBox::new_zero_with_align(self, align, direction)
    }

    pub fn map_single_array<T>(
        &self,
        buff: &[T],
        align: usize,
        direction: DmaDirection,
    ) -> Result<SArrayPtr<T>, DmaError> {
        SArrayPtr::map_single(self, buff, align, direction)
    }
}
