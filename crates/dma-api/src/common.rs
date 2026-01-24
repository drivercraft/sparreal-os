use core::{alloc::Layout, num::NonZeroUsize, ptr::NonNull};

use crate::{DeviceDma, DmaDirection, DmaError, DmaHandle};

pub(crate) struct DCommon {
    pub handle: DmaHandle,
    pub osal: DeviceDma,
    pub direction: DmaDirection,
}

unsafe impl Send for DCommon {}

impl DCommon {
    pub fn new_zero(
        os: &DeviceDma,
        layout: Layout,
        direction: DmaDirection,
    ) -> Result<Self, DmaError> {
        let handle = unsafe { os.alloc_coherent(layout) }?;
        let ptr = handle.dma_virt();
        unsafe {
            ptr.write_bytes(0, handle.size());
        }
        os.flush_invalidate(ptr, handle.size());

        Ok(Self {
            handle,
            osal: os.clone(),
            direction,
        })
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(self.handle.dma_virt().as_ptr(), self.handle.size())
        }
    }

    pub fn prepare_read(&self, offset: usize, size: usize) {
        self.osal
            .prepare_read(&self.handle, offset, size, self.direction);
    }

    pub fn confirm_write(&self, offset: usize, size: usize) {
        self.osal
            .confirm_write(&self.handle, offset, size, self.direction);
    }

    pub fn dma_ptr(&self, offset: usize) -> *mut u8 {
        let ptr = unsafe { self.handle.dma_virt().add(offset) };
        ptr.as_ptr()
    }

    pub fn confirm_write_all(&self) {
        self.osal
            .confirm_write(&self.handle, 0, self.handle.size(), self.direction);
    }
}

impl Drop for DCommon {
    fn drop(&mut self) {
        if self.handle.size() > 0 {
            unsafe {
                self.osal.dealloc_coherent(self.handle);
            }
        }
    }
}

pub struct SingleMap {
    pub handle: DmaHandle,
    osal: DeviceDma,
    pub direction: DmaDirection,
}

unsafe impl Send for SingleMap {}

impl SingleMap {
    pub(crate) fn new(
        os: &DeviceDma,
        addr: NonNull<u8>,
        size: NonZeroUsize,
        align: usize,
        direction: DmaDirection,
    ) -> Result<Self, DmaError> {
        let handle = unsafe { os._map_single(addr, size, align, direction)? };

        Ok(Self {
            handle,
            osal: os.clone(),
            direction,
        })
    }

    pub(crate) fn new_from_slice<T>(
        os: &DeviceDma,
        buff: &[T],
        align: usize,
        direction: DmaDirection,
    ) -> Result<Self, DmaError> {
        let addr = NonNull::new(buff.as_ptr() as *mut u8).ok_or(DmaError::NullPointer)?;
        let size =
            NonZeroUsize::new(core::mem::size_of_val(buff)).ok_or(DmaError::ZeroSizedBuffer)?;

        Self::new(os, addr, size, align, direction)
    }

    pub fn len(&self) -> usize {
        self.handle.size()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 获取 DMA 地址
    pub fn dma_addr(&self) -> crate::DmaAddr {
        self.handle.dma_addr
    }

    pub fn prepare_read_all(&self) {
        self.osal
            .prepare_read(&self.handle, 0, self.len(), self.direction);
    }

    pub fn confirm_write_all(&self) {
        self.osal
            .confirm_write(&self.handle, 0, self.len(), self.direction);
    }
}

impl Drop for SingleMap {
    fn drop(&mut self) {
        unsafe {
            self.osal.unmap_single(self.handle);
        }
    }
}
