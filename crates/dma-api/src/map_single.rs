use core::{num::NonZeroUsize, ptr::NonNull};

use alloc::vec::Vec;

use crate::{DeviceDma, DmaDirection, DmaError, DmaMapHandle};

/// A simple DMA array that maps a single contiguous memory region.
pub struct SArrayPtr<T> {
    handle: DmaMapHandle,
    osal: DeviceDma,
    pub direction: DmaDirection,
    _marker: core::marker::PhantomData<*mut T>,
}

impl<T> SArrayPtr<T> {
    /// Create a new SArrayPtr from a raw pointer and size.
    pub(crate) fn map_single(
        os: &DeviceDma,
        buff: &[T],
        align: usize,
        direction: DmaDirection,
    ) -> Result<Self, DmaError> {
        let addr = NonNull::new(buff.as_ptr() as *mut u8).ok_or(DmaError::NullPointer)?;
        let size =
            NonZeroUsize::new(core::mem::size_of_val(buff)).ok_or(DmaError::ZeroSizedBuffer)?;
        let handle = unsafe { os._map_single(addr, size, align, direction)? };

        Ok(Self {
            handle,
            osal: os.clone(),
            direction,
            _marker: core::marker::PhantomData,
        })
    }

    pub fn copy_from_slice(&mut self, src: &[T]) {
        assert!(
            core::mem::size_of_val(src) <= self.handle.size(),
            "Source slice is larger than DMA buffer"
        );
        unsafe {
            let dest_ptr = self.handle.cpu_addr.cast::<T>();
            dest_ptr
                .as_ptr()
                .copy_from_nonoverlapping(src.as_ptr(), src.len());
        }
        self.osal
            .confirm_write(&self.handle, 0, self.handle.size(), self.direction);
    }

    pub fn dma_addr(&self) -> crate::DmaAddr {
        self.handle.dma_addr
    }

    pub fn len(&self) -> usize {
        self.handle.size() / core::mem::size_of::<T>()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn read(&self, index: usize) -> Option<T> {
        if index >= self.len() {
            return None;
        }

        unsafe {
            let offset = index * core::mem::size_of::<T>();
            self.osal.prepare_read(
                &self.handle,
                offset,
                core::mem::size_of::<T>(),
                self.direction,
            );
            let ptr = self.handle.cpu_addr.cast::<T>().add(index);
            Some(ptr.read())
        }
    }

    pub fn set(&mut self, index: usize, value: T) {
        assert!(
            index < self.len(),
            "index out of range, index: {},len: {}",
            index,
            self.len()
        );

        unsafe {
            let ptr = self.handle.cpu_addr.cast::<T>().add(index);
            ptr.write(value);
        }

        self.osal.confirm_write(
            &self.handle,
            index * core::mem::size_of::<T>(),
            core::mem::size_of::<T>(),
            self.direction,
        );
    }

    pub fn to_vec(&self) -> Vec<T> {
        let mut vec: Vec<T> = Vec::with_capacity(self.len());
        self.osal
            .prepare_read(&self.handle, 0, self.handle.size(), self.direction);
        unsafe {
            let src_ptr = self.handle.cpu_addr.as_ptr().cast::<T>();
            vec.set_len(self.len());
            let dst_ptr = vec.as_mut_ptr();
            dst_ptr.copy_from_nonoverlapping(src_ptr, self.len());
        }
        vec
    }
}

impl<T> Drop for SArrayPtr<T> {
    fn drop(&mut self) {
        unsafe {
            self.osal.unmap_single(self.handle);
        }
    }
}
