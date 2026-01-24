use core::ptr::NonNull;

use crate::{DeviceDma, DmaAddr, DmaDirection, DmaError, common::DCommon};

pub struct DBox<T> {
    data: DCommon,
    _marker: core::marker::PhantomData<T>,
}

unsafe impl<T> Send for DBox<T> where T: Send {}

impl<T> DBox<T> {
    pub(crate) fn new_zero(os: &DeviceDma, direction: DmaDirection) -> Result<Self, DmaError> {
        let layout = core::alloc::Layout::from_size_align(
            core::mem::size_of::<T>(),
            core::mem::align_of::<T>(),
        )?;
        let data = DCommon::new_zero(os, layout, direction)?;
        Ok(Self {
            data,
            _marker: core::marker::PhantomData,
        })
    }

    pub(crate) fn new_zero_with_align(
        os: &DeviceDma,
        align: usize,
        direction: DmaDirection,
    ) -> Result<Self, DmaError> {
        let layout = core::alloc::Layout::from_size_align(
            core::mem::size_of::<T>(),
            align.max(core::mem::align_of::<T>()),
        )?;
        let data = DCommon::new_zero(os, layout, direction)?;
        Ok(Self {
            data,
            _marker: core::marker::PhantomData,
        })
    }

    pub fn dma_addr(&self) -> DmaAddr {
        self.data.handle.dma_addr
    }

    pub fn read(&self) -> T {
        unsafe {
            self.data.prepare_read(0, core::mem::size_of::<T>());
            let ptr = self.data.handle.cpu_addr.cast::<T>();
            ptr.read()
        }
    }

    pub fn write(&mut self, value: T) {
        unsafe {
            let ptr = self.data.handle.cpu_addr.cast::<T>();
            ptr.write(value);
            self.data.confirm_write(0, core::mem::size_of::<T>());
        }
    }

    pub fn modify(&mut self, f: impl FnOnce(&mut T)) {
        let mut value = self.read();
        f(&mut value);
        self.write(value);
    }

    pub fn as_ptr(&self) -> NonNull<T> {
        self.data.handle.as_ptr().cast::<T>()
    }

    /// 获取底层缓冲区的可变切片
    ///
    /// # Safety
    ///
    /// - 调用者必须确保在使用该切片期间，设备不会访问此内存区域
    /// - 调用者必须手动处理缓存同步（flush/invalidate）
    pub unsafe fn as_buff_mut(&mut self) -> &mut [u8] {
        self.data.as_mut_slice()
    }
}
