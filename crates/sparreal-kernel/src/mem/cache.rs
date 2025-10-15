use core::ptr::NonNull;

use dma_api::Osal;

use crate::platform::{self, CacheOp};

use super::{PhysAddr, VirtAddr};

struct DMAImpl;

impl Osal for DMAImpl {
    fn map(&self, addr: NonNull<u8>, _size: usize, _direction: dma_api::Direction) -> u64 {
        let vaddr = VirtAddr::from(addr);
        let paddr = PhysAddr::from(vaddr);
        paddr.raw() as _
    }

    fn unmap(&self, _addr: NonNull<u8>, _size: usize) {}

    unsafe fn alloc(&self, dma_mask: u64, layout: core::alloc::Layout) -> *mut u8 {
        #[cfg(target_os = "none")]
        {
            unsafe { super::ALLOCATOR.alloc_with_mask(layout, dma_mask) }
        }

        #[cfg(not(target_os = "none"))]
        {
            let _ = dma_mask;
            unsafe { alloc::alloc::alloc(layout) }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        #[cfg(target_os = "none")]
        {
            unsafe { core::alloc::GlobalAlloc::dealloc(&super::ALLOCATOR, ptr, layout) };
        }

        #[cfg(not(target_os = "none"))]
        {
            unsafe { alloc::alloc::dealloc(ptr, layout) }
        }
    }
}

pub fn init() {
    unsafe {
        dma_api::init(&DMAImpl);
    }
}
