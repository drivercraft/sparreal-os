#[macro_use]
mod _macros;

#[cfg(feature = "hv")]
#[path = "el2/mod.rs"]
mod elx;

#[cfg(not(feature = "hv"))]
#[path = "el1/mod.rs"]
mod elx;

mod addrspace;
mod context;
mod entry;
mod head;
pub(crate) mod irq;
pub mod paging;
mod power;
pub mod relocate;
mod trap;

use aarch64_cpu::registers::*;
use elx::*;
pub use paging::Entry;

use crate::{
    ArchTrait,
    arch::{addrspace::PAGE_OFFSET, trap::trap_addr},
    consts::VM_LOAD_ADDRESS,
    mem::{__kimage_va_to_pa, PageTableInfo},
};

pub struct Arch;

impl ArchTrait for Arch {
    type P = paging::Generic;

    fn _va(paddr: usize) -> *mut u8 {
        (paddr + PAGE_OFFSET) as *mut u8
    }

    fn _percpu(paddr: usize) -> *mut u8 {
        (paddr + PAGE_OFFSET + 0xFF00_0000_0000) as *mut u8
    }

    fn post_allocator() {
        power::init();
    }

    fn per_cpu_trap_init(_is_primary: bool) {
        trap::setup();
        println!("Disable user page table");
        #[cfg(uspace)]
        elx::set_user_table(PageTableInfo { asid: 0, addr: 0 });
        elx::flush_tlb(None);
    }

    fn systimer_enable() {
        elx::systick_enable();
    }

    fn systimer_irq_disable() {
        // debug!("Disable systick irq");
        elx::systick_irq_disable();
    }

    fn systimer_irq_enable() {
        // debug!("Enable systick irq");
        elx::systick_irq_enable();
    }

    fn systimer_irq_is_enabled() -> bool {
        elx::systick_irq_is_enabled()
    }

    fn systimer_set_interval(ticks: usize) {
        elx::systick_set_interval(ticks);
    }

    fn systimer_ack() {
        // ARM generic timer doesn't need explicit ACK
        // The interrupt is cleared when a new timer value is set
    }

    fn systimer_freq() -> usize {
        CNTFRQ_EL0.get() as _
    }

    fn systimer_tick() -> usize {
        CNTPCT_EL0.get() as _
    }

    fn shutdown() -> ! {
        power::shutdown()
    }

    fn irq_all_is_enabled() -> bool {
        !DAIF.is_set(DAIF::I)
    }

    fn irq_all_set_enable(enable: bool) {
        DAIF.modify(if enable {
            DAIF::I::CLEAR
        } else {
            DAIF::I::Masked
        });
    }

    fn kernel_page_table() -> PageTableInfo {
        elx::get_kernal_table()
    }

    fn set_kernel_page_table(val: PageTableInfo) {
        elx::set_kernal_table(val);
        elx::flush_tlb(None);
    }

    #[cfg(uspace)]
    fn user_page_table() -> PageTableInfo {
        elx::get_user_table()
    }

    #[cfg(uspace)]
    fn set_user_page_table(val: PageTableInfo) {
        elx::set_user_table(val);
        elx::flush_tlb(None);
    }

    fn irq_is_enabled(_irq: crate::irq::IrqId) -> bool {
        unimplemented!()
    }

    fn irq_set_enable(_irq: crate::irq::IrqId, _enable: bool) {
        unimplemented!()
    }

    fn virt_to_phys(vaddr: *const u8) -> usize {
        if is_mmu_enabled() {
            if vaddr as usize >= VM_LOAD_ADDRESS {
                __kimage_va_to_pa(vaddr)
            } else {
                vaddr as usize & 0xffff_ffff_ffff
            }
        } else {
            vaddr as usize
        }
    }

    fn trap_addr() -> usize {
        trap_addr()
    }

    fn jump_to(entry: usize, sp: usize) -> ! {
        unsafe {
            core::arch::asm!(
                "mov sp, {sp}",
                "br {entry}",
                sp = in(reg) sp,
                entry = in(reg) entry,
                options(noreturn)
            );
        }
    }
}
