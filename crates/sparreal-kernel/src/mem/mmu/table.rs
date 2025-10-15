use page_table_generic::PagingError;
use spin::Mutex;

#[cfg(target_os = "none")]
use crate::mem::ALLOCATOR;
use crate::{
    hal_al::mmu::MapConfig,
    irq::NoIrqGuard,
    mem::{
        Phys, PhysAddr, VirtAddr,
        mmu::{AccessSetting, CacheSetting, HeapGuard},
    },
    platform,
};
static KERNEL_TABLE: Mutex<Option<PageTable>> = Mutex::new(None);

pub(crate) fn set_kernal_table(table: PageTable) {
    let g = NoIrqGuard::new();
    let mut guard = KERNEL_TABLE.lock();
    if guard.is_some() {
        panic!("kernel table is already set");
    }
    platform::mmu::set_kernel_table(table.raw);
    *guard = Some(table);
    drop(g);
}

pub fn replace_kernel_table(new: PageTable) -> Option<PageTable> {
    let g = NoIrqGuard::new();
    let mut guard = KERNEL_TABLE.lock();
    let current = platform::mmu::get_kernel_table();
    platform::mmu::set_kernel_table(new.raw);
    let mut old = guard.replace(new);
    if old.is_none() {
        old = Some(unsafe { PageTable::raw_to_own(current) });
    }
    drop(g);
    old
}

pub fn new_table() -> Result<PageTable, PagingError> {
    let mut g = ALLOCATOR.lock_heap32();
    let mut access = HeapGuard(g);
    let raw = platform::mmu::new_table(&mut access)?;
    Ok(unsafe { PageTable::raw_to_own(raw) })
}

pub fn with_kernel_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut PageTable) -> R,
{
    let g = NoIrqGuard::new();
    let mut guard = KERNEL_TABLE.lock();
    if let Some(ref mut table) = *guard {
        let r = f(table);
        drop(g);
        r
    } else {
        panic!("kernel table is not initialized");
    }
}

pub struct PageTable {
    raw: crate::hal_al::mmu::PageTableRef,
}

impl PageTable {
    pub(crate) unsafe fn raw_to_own(raw: crate::hal_al::mmu::PageTableRef) -> Self {
        Self { raw }
    }

    pub fn id(&self) -> usize {
        self.raw.id
    }

    pub fn addr(&self) -> Phys<u8> {
        self.raw.addr
    }

    pub fn map(&mut self, config: &MapConfig) -> Result<(), PagingError> {
        let mut g = ALLOCATOR.lock_heap32();
        let mut access = HeapGuard(g);
        platform::mmu::table_map(self.raw, &mut access, config)
    }
}

impl Drop for PageTable {
    fn drop(&mut self) {
        let mut g = ALLOCATOR.lock_heap32();
        let mut access = HeapGuard(g);
        platform::mmu::release_table(self.raw, &mut access);
    }
}
