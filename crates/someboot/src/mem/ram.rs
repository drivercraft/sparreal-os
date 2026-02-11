use core::{alloc::Layout, ops::Range};

use num_align::NumAlign;
use page_table_generic::FrameAllocator;

use crate::mem::page_size;

/// RAM 分配器的起始地址
static mut RAM_START: usize = 0;

/// RAM 分配器的结束地址
static mut RAM_END: usize = 0;

/// 当前分配位置
static mut RAM_CURRENT: usize = 0;

/// 简单的线性内存分配器
///
/// # Safety
/// 此函数仅应在单核环境下的早期启动阶段使用
pub unsafe fn alloc(layout: Layout) -> Option<usize> {
    let start = unsafe { RAM_CURRENT.align_up(layout.align()) };
    let end = start + layout.size();

    if end > unsafe { RAM_END } {
        return None;
    }

    unsafe {
        RAM_CURRENT = end;
    }
    Some(start)
}

/// 获取当前分配位置
pub fn current() -> *mut u8 {
    unsafe { RAM_CURRENT as _ }
}

/// 初始化 RAM 分配器
pub fn init(range: Range<usize>) {
    println!("Initialize RAM allocator: {:#x?}", range);
    unsafe {
        RAM_START = range.start;
        RAM_END = range.end;
        RAM_CURRENT = range.start.max(0x40);
    }
}

/// 获取已使用的内存范围
pub fn used_range() -> Range<usize> {
    unsafe {
        let start = RAM_START;
        let end = RAM_CURRENT;
        start..end.align_up(page_size())
    }
}

/// RAM 分配器类型（保留用于 FrameAllocator trait）
#[derive(Clone, Copy)]
pub struct Ram;

impl Ram {
    pub fn current(&self) -> *mut u8 {
        current()
    }

    pub fn alloc(&self, layout: Layout) -> Option<usize> {
        unsafe { alloc(layout) }
    }
}

impl FrameAllocator for Ram {
    fn alloc_frame(&self) -> Option<page_table_generic::PhysAddr> {
        unsafe {
            alloc(Layout::from_size_align_unchecked(page_size(), page_size())).map(|ptr| ptr.into())
        }
    }

    fn dealloc_frame(&self, _frame: page_table_generic::PhysAddr) {}

    fn phys_to_virt(&self, paddr: page_table_generic::PhysAddr) -> *mut u8 {
        super::phys_to_virt(paddr.raw())
    }
}
