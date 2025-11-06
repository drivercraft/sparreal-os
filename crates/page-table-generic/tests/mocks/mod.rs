use std::{
    alloc::{self, Layout},
    fmt::Debug,
    mem,
};

use page_table_generic::*;
use tock_registers::{interfaces::*, register_bitfields, registers::*};

pub const MB: usize = 1024 * 1024;
pub const GB: usize = 1024 * MB;

register_bitfields! [
    u64,
    PTE64 [
        PA OFFSET(0) NUMBITS(48) [
        ],
        READ OFFSET(48) NUMBITS(1) [
        ],
        WRITE OFFSET(49) NUMBITS(1) [
        ],
        USER_EXECUTE OFFSET(50) NUMBITS(1) [
        ],
        USER_ACCESS OFFSET(51) NUMBITS(1) [
        ],
        PRIVILEGE_EXECUTE OFFSET(52) NUMBITS(1) [
        ],
        BLOCK OFFSET(53) NUMBITS(1) [
        ],
        CACHE OFFSET(54) NUMBITS(2) [
            NonCache = 0,
            Normal = 0b01,
            Device = 0b10,
        ],
        VALID OFFSET(63) NUMBITS(1) [

        ]
    ],
];

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PteImpl(pub u64);

impl PteImpl {
    fn reg(&self) -> &ReadWrite<u64, PTE64::Register> {
        unsafe { mem::transmute(self) }
    }
}

impl Debug for PteImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.valid() {
            return write!(f, "invalid");
        }

        write!(f, "PTE PA: {:?} Block: {:?}", self.paddr(), self.is_huge())
    }
}

impl PageTableEntry for PteImpl {
    fn valid(&self) -> bool {
        self.reg().is_set(PTE64::VALID)
    }

    fn paddr(&self) -> PhysAddr {
        ((self.reg().read(PTE64::PA) << 12) as usize).into()
    }

    fn set_paddr(&mut self, paddr: PhysAddr) {
        let paddr = paddr.raw() >> 12;
        self.reg().modify(PTE64::PA.val(paddr as _));
    }

    fn set_valid(&mut self, valid: bool) {
        self.reg().modify(if valid {
            PTE64::VALID::SET
        } else {
            PTE64::VALID::CLEAR
        });
    }

    fn is_huge(&self) -> bool {
        self.reg().is_set(PTE64::BLOCK)
    }

    fn set_is_huge(&mut self, is_block: bool) {
        self.reg().modify(if is_block {
            PTE64::BLOCK::SET
        } else {
            PTE64::BLOCK::CLEAR
        });
    }
}

pub type Tb4kL4 = PageTable<T4kL4, crate::mocks::Fram4k>;

#[derive(Debug, Clone, Copy)]
pub struct T4kL4;

impl TableGeneric for T4kL4 {
    type P = PteImpl;

    const PAGE_SIZE: usize = 0x1000;

    const LEVEL: usize = 4;

    const MAX_BLOCK_LEVEL: usize = 3;

    fn flush(vaddr: Option<VirtAddr>) {
        let _ = vaddr;
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Fram4k;

impl FramAllocator for Fram4k {
    fn alloc_frame(&self) -> Option<PhysAddr> {
        let layout = Layout::from_size_align(4096, 4096).unwrap();
        let ptr = unsafe { alloc::alloc(layout) };
        if ptr.is_null() {
            None
        } else {
            Some(PhysAddr::new(ptr as usize))
        }
    }

    fn dealloc_frame(&self, frame: PhysAddr) {
        let layout = Layout::from_size_align(4096, 4096).unwrap();
        unsafe {
            alloc::dealloc(frame.raw() as *mut u8, layout);
        }
    }

    fn phys_to_virt(&self, paddr: PhysAddr) -> *mut u8 {
        paddr.raw() as *mut u8
    }
}
