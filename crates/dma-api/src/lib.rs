#![cfg_attr(target_os = "none", no_std)]
#![doc = include_str!("../README.md")]

extern crate alloc;

use core::{alloc::Layout, num::NonZeroUsize, ops::Deref, ptr::NonNull};

mod osal;

mod array;
mod common;
mod dbox;
mod def;

pub use array::*;
pub use common::SingleMap;
pub use dbox::*;
pub use def::*;
pub use osal::DmaOp;

/// Handle for DMA memory allocation.
///
/// Manages DMA memory buffers that may require special alignment or DMA address mask
/// constraints. When the original virtual address doesn't meet alignment or mask
/// requirements, an additional aligned buffer is allocated and stored in `alloc_virt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DmaHandle {
    /// Original virtual address pointer (may not be aligned)
    pub(crate) origin_virt: NonNull<u8>,
    /// DMA address visible to devices
    pub(crate) dma_addr: DmaAddr,
    /// Memory layout specification (size and alignment)
    pub(crate) layout: Layout,
    /// Additional allocated virtual address if the original doesn't satisfy
    /// alignment or DMA mask requirements
    pub(crate) alloc_virt: Option<NonNull<u8>>,
}
impl DmaHandle {
    /// 为 `alloc_coherent` 操作创建 `DmaHandle`。
    ///
    /// 此构造函数专门用于 DMA 一致性内存分配场景，其中：
    /// - 内存是专门为 DMA 分配的（零初始化）
    /// - CPU 和设备看到同一个虚拟地址
    /// - 不需要额外的对齐缓冲区
    ///
    /// # 特性保证
    ///
    /// - `alloc_virt` 总是 `None`（无需额外分配）
    /// - `origin_virt == dma_virt`（地址同一性）
    /// - 内存已被零初始化
    ///
    /// # Safety
    ///
    /// 调用者必须确保：
    /// - `origin_virt` 指向有效内存，生命周期与 handle 相同
    /// - `dma_addr` 是与 `origin_virt` 对应的设备可访问地址
    /// - `layout` 正确描述内存的大小和对齐
    /// - 内存必须保持有效直到被正确释放
    ///
    /// # 使用场景
    ///
    /// 此构造函数应在实现 `DmaOp::alloc_coherent` 时使用：
    ///
    /// ```rust,ignore
    /// unsafe fn alloc_coherent(
    ///     &self,
    ///     _dma_mask: u64,
    ///     layout: Layout,
    /// ) -> Option<DmaHandle> {
    ///     let ptr = unsafe { alloc_zeroed(layout) };
    ///     if ptr.is_null() {
    ///         return None;
    ///     }
    ///     Some(unsafe {
    ///         DmaHandle::new_for_alloc_coherent(
    ///             NonNull::new(ptr).unwrap(),
    ///             (ptr as u64).into(),
    ///             layout
    ///         )
    ///     })
    /// }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `origin_virt` - 虚拟地址指针（也是 DMA 虚拟地址）
    /// * `dma_addr` - 设备可见的 DMA 地址
    /// * `layout` - 内存布局（大小和对齐）
    pub unsafe fn new_for_alloc_coherent(
        origin_virt: NonNull<u8>,
        dma_addr: DmaAddr,
        layout: Layout,
    ) -> Self {
        Self {
            origin_virt,
            dma_addr,
            layout,
            alloc_virt: None, // 固定为 None
        }
    }

    /// 为 `map_single` 操作创建 `DmaHandle`。
    ///
    /// 此构造函数专门用于映射现有内存到 DMA 地址的场景，其中：
    /// - 内存已存在（不是为此 DMA 操作新分配的）
    /// - 可能需要额外的对齐缓冲区来满足 DMA 要求
    /// - `origin_virt` 和 `dma_virt` 可能不同
    ///
    /// # 特性说明
    ///
    /// - `alloc_virt` 可能是 `Some(virt)`（当原始地址不对齐时）
    /// - `origin_virt` 是用户提供的原始地址
    /// - `dma_virt` 是实际用于 DMA 的地址（可能是 `alloc_virt` 或 `origin_virt`）
    ///
    /// # Safety
    ///
    /// 调用者必须确保：
    /// - `origin_virt` 指向有效的现有内存
    /// - `dma_addr` 是与实际 DMA 虚拟地址对应的设备地址
    /// - `layout` 正确描述内存的大小和对齐
    /// - 如果 `alloc_virt` 是 `Some(v)`，则 `v` 必须指向有效分配的对齐缓冲区
    /// - 如果提供了 `alloc_virt`，必须在 `prepare_read` 时从 `dma_virt` 复制到 `origin_virt`
    /// - 如果提供了 `alloc_virt`，必须在 `confirm_write` 时从 `origin_virt` 复制到 `dma_virt`
    ///
    /// # 使用场景
    ///
    /// 此构造函数应在实现 `DmaOp::map_single` 时使用：
    ///
    /// ```rust,ignore
    /// unsafe fn map_single(
    ///     &self,
    ///     _dma_mask: u64,
    ///     addr: NonNull<u8>,
    ///     size: NonZeroUsize,
    ///     align: usize,
    ///     _direction: DmaDirection,
    /// ) -> Result<DmaHandle, DmaError> {
    ///     let layout = Layout::from_size_align(size.get(), align)?;
    ///
    ///     // 检查原始地址是否对齐
    ///     if addr.as_ptr() as usize % align == 0 {
    ///         // 原始地址已对齐，无需额外分配
    ///         Ok(unsafe {
    ///             DmaHandle::new_for_map_single(
    ///                 addr,
    ///                 (addr.as_ptr() as u64).into(),
    ///                 layout,
    ///                 None,
    ///             )
    ///         })
    ///     } else {
    ///         // 分配对齐的缓冲区
    ///         let aligned = alloc_aligned(layout);
    ///         Ok(unsafe {
    ///             DmaHandle::new_for_map_single(
    ///                 addr,
    ///                 (aligned.as_ptr() as u64).into(),
    ///                 layout,
    ///                 Some(aligned),
    ///             )
    ///         })
    ///     }
    /// }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `origin_virt` - 用户提供的原始虚拟地址
    /// * `dma_addr` - 设备可见的 DMA 地址（对应实际 DMA 虚拟地址）
    /// * `layout` - 内存布局（大小和对齐）
    /// * `alloc_virt` - 可选的对齐缓冲区虚拟地址
    pub unsafe fn new_for_map_single(
        origin_virt: NonNull<u8>,
        dma_addr: DmaAddr,
        layout: Layout,
        alloc_virt: Option<NonNull<u8>>,
    ) -> Self {
        Self {
            origin_virt,
            dma_addr,
            layout,
            alloc_virt, // 显式传入
        }
    }

    /// Returns the size of the DMA buffer in bytes.
    pub fn size(&self) -> usize {
        self.layout.size()
    }

    /// Returns the alignment requirement of the DMA buffer in bytes.
    pub fn align(&self) -> usize {
        self.layout.align()
    }

    /// Returns the virtual address to access data.
    pub fn as_ptr(&self) -> NonNull<u8> {
        self.origin_virt
    }

    /// Returns the virtual address used for actual DMA operations.
    ///
    /// This is either `alloc_virt` if an additional buffer was allocated,
    /// or `origin_virt` otherwise.
    pub(crate) fn dma_virt(&self) -> NonNull<u8> {
        if let Some(virt) = self.alloc_virt {
            virt
        } else {
            self.origin_virt
        }
    }

    /// Returns the DMA address visible to devices.
    pub fn dma_addr(&self) -> DmaAddr {
        self.dma_addr
    }

    /// Returns the memory layout used for this DMA allocation.
    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// Returns the additional allocated virtual address, if present.
    ///
    /// This returns `Some` when the original address didn't meet alignment
    /// or DMA mask requirements, and an extra aligned buffer was allocated.
    pub fn alloc_virt(&self) -> Option<NonNull<u8>> {
        self.alloc_virt
    }
}
unsafe impl Send for DmaHandle {}

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
        handle: &DmaHandle,
        offset: usize,
        size: usize,
        direction: DmaDirection,
    ) {
        self.os.prepare_read(handle, offset, size, direction)
    }

    fn confirm_write(
        &self,
        handle: &DmaHandle,
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

        let is_aligned = handle.dma_addr.as_u64().is_multiple_of(self.dma_mask());
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
    ) -> Result<DmaHandle, DmaError> {
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

    unsafe fn unmap_single(&self, handle: DmaHandle) {
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

    pub fn map_single(
        &self,
        addr: NonNull<u8>,
        size: NonZeroUsize,
        align: usize,
        direction: DmaDirection,
    ) -> Result<common::SingleMap, DmaError> {
        common::SingleMap::new(self, addr, size, align, direction)
    }
}
