use core::ptr::NonNull;

use crate::{Direction, DmaError, DmaHandle, MapHandle};

cfg_if::cfg_if! {
    if #[cfg(target_arch = "aarch64")] {
        #[path = "aarch64.rs"]
        pub mod arch;
    } else{
        #[path = "nop.rs"]
        pub mod arch;
    }
}

pub trait Osal: Sync + Send + 'static {
    fn page_size(&self) -> usize;

    /// 将虚拟地址映射到 DMA 地址
    /// 
    /// # Safety
    /// 只能是单个连续内存块
    unsafe fn map_single(
        &self,
        dma_mask: u64,
        addr: NonNull<u8>,
        size: usize,
        direction: Direction,
    ) -> Result<MapHandle, DmaError>;

    /// 解除 DMA 映射
    /// 
    /// # Safety
    /// 必须与 map_single 配对使用
    unsafe fn unmap_single(&self, handle: MapHandle);

    /// 写回缓存到内存 (clean)
    fn flush(&self, addr: NonNull<u8>, size: usize) {
        arch::flush(addr, size)
    }

    /// 使缓存无效 (invalidate)
    fn invalidate(&self, addr: NonNull<u8>, size: usize) {
        arch::invalidate(addr, size)
    }

    /// 分配 DMA 可访问内存
    /// # Safety
    ///
    /// - 调用者必须确保 layout 合法
    /// - 返回的内存必须保证连续
    unsafe fn alloc_coherent(
        &self,
        dma_mask: u64,
        layout: core::alloc::Layout,
    ) -> Option<DmaHandle>;

    /// 释放 DMA 内存
    /// # Safety
    /// 调用者必须确保 ptr 和 layout 与 alloc 时匹配
    unsafe fn dealloc_coherent(&self, dma_mask: u64, handle: DmaHandle);

    fn prepare_read(&self, ptr: NonNull<u8>, size: usize, direction: Direction) {
        if matches!(direction, Direction::FromDevice | Direction::Bidirectional) {
            self.invalidate(ptr, size);
        }
    }

    fn confirm_write(&self, ptr: NonNull<u8>, size: usize, direction: Direction) {
        if matches!(direction, Direction::ToDevice | Direction::Bidirectional) {
            self.flush(ptr, size)
        }
    }
}
