use core::hint::spin_loop;

use page_table_generic::{PageTableEntry, PteConfig, TableMeta, VirtAddr};

use crate::{ArchTrait, DCacheOp, mem::PageTableInfo, power::CpuOnError};

pub(crate) mod irq;

#[derive(Clone, Copy, Debug)]
pub struct Entry;

impl PageTableEntry for Entry {
    fn from_config(_config: PteConfig) -> Self {
        Self
    }

    fn to_config(&self, is_dir: bool) -> PteConfig {
        PteConfig {
            is_dir,
            ..Default::default()
        }
    }

    fn valid(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy)]
pub struct Generic;

impl TableMeta for Generic {
    type P = Entry;

    const PAGE_SIZE: usize = 0x1000;
    const LEVEL_BITS: &'static [usize] = &[9, 9, 9, 9];
    const MAX_BLOCK_LEVEL: usize = 1;

    fn flush(_vaddr: Option<VirtAddr>) {}
}

pub struct Arch;

impl ArchTrait for Arch {
    type P = Generic;

    fn _va(paddr: usize) -> *mut u8 {
        paddr as *mut u8
    }

    fn cpu_current_hartid() -> usize {
        0
    }

    fn jump_to(_entry: usize, _sp: usize) -> ! {
        loop {
            spin_loop();
        }
    }

    fn post_allocator() {}

    fn per_cpu_trap_init(_is_primary: bool) {}

    fn trap_addr() -> usize {
        0
    }

    fn virt_to_phys(vaddr: *const u8) -> usize {
        vaddr as usize
    }

    fn kernel_space() -> core::ops::Range<usize> {
        0..usize::MAX
    }

    fn kernel_page_table() -> PageTableInfo {
        PageTableInfo { asid: 0, addr: 0 }
    }

    fn set_kernel_page_table(_val: PageTableInfo) {}

    #[cfg(uspace)]
    fn user_page_table() -> PageTableInfo {
        PageTableInfo { asid: 0, addr: 0 }
    }

    #[cfg(uspace)]
    fn set_user_page_table(_val: PageTableInfo) {}

    fn shutdown() -> ! {
        loop {
            spin_loop();
        }
    }

    fn secondary_entry_fn_address() -> *const () {
        secondary_entry_placeholder as *const ()
    }

    fn cpu_on(_hartid: usize, _entry: usize, _arg: usize) -> Result<(), CpuOnError> {
        Err(CpuOnError::NotSupported)
    }

    fn systimer_enable() {}

    fn systimer_irq_enable() {}

    fn systimer_irq_disable() {}

    fn systimer_irq_is_enabled() -> bool {
        false
    }

    fn systimer_set_interval(_ticks: usize) {}

    fn systimer_ack() {}

    fn systimer_freq() -> usize {
        0
    }

    fn systimer_tick() -> usize {
        0
    }

    fn irq_all_is_enabled() -> bool {
        false
    }

    fn irq_all_set_enable(_enable: bool) {}

    fn irq_is_enabled(_irq: crate::irq::IrqId) -> bool {
        false
    }

    fn irq_set_enable(_irq: crate::irq::IrqId, _enable: bool) {}

    fn dcache_range(_op: DCacheOp, _addr: usize, _size: usize) {}
}

extern "C" fn secondary_entry_placeholder(_arg: usize) -> ! {
    loop {
        spin_loop();
    }
}
