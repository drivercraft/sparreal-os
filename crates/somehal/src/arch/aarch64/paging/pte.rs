use page_table_generic::{PageTableEntry, TableGeneric};

bitflags::bitflags! {
    #[repr(transparent)]
    /// Memory attribute fields in the VMSAv8-64 translation table format descriptors.
    #[derive(Clone, Copy)]
    pub struct PteFlags: usize {
        // Attribute fields in stage 1 VMSAv8-64 Block and Page descriptors:

        /// Whether the descriptor is valid.
        const VALID =       1 << 0;
        /// The descriptor gives the address of the next level of translation table or 4KB page.
        /// (not a 2M, 1G block)
        const NON_BLOCK =   1 << 1;

        /// Non-secure bit. For memory accesses from Secure state, specifies whether the output
        /// address is in Secure or Non-secure memory.
        const NS =          1 << 5;
        /// Access permission: accessable at EL0.
        const AP_EL0 =      1 << 6;
        /// Access permission: read-only.
        const AP_RO =       1 << 7;
        /// Shareability: Inner Shareable (otherwise Outer Shareable).
        const INNER =       1 << 8;
        /// Shareability: Inner or Outer Shareable (otherwise Non-shareable).
        const SHAREABLE =   1 << 9;
        /// The Access flag.
        const AF =          1 << 10;
        /// The not global bit.
        const NG =          1 << 11;
        /// Indicates that 16 adjacent translation table entries point to contiguous memory regions.
        const CONTIGUOUS =  1 <<  52;
        /// The Privileged execute-never field.
        const PXN =         1 <<  53;
        /// The Execute-never or Unprivileged execute-never field.
        const UXN =         1 <<  54;

        // Next-level attributes in stage 1 VMSAv8-64 Table descriptors:

        /// PXN limit for subsequent levels of lookup.
        const PXN_TABLE =           1 << 59;
        /// XN limit for subsequent levels of lookup.
        const XN_TABLE =            1 << 60;
        /// Access permissions limit for subsequent levels of lookup: access at EL0 not permitted.
        const AP_NO_EL0_TABLE =     1 << 61;
        /// Access permissions limit for subsequent levels of lookup: write access not permitted.
        const AP_NO_WRITE_TABLE =   1 << 62;
        /// For memory accesses from Secure state, specifies the Security state for subsequent
        /// levels of lookup.
        const NS_TABLE =            1 << 63;
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Entry(usize);

impl Entry {
    const PHYS_ADDR_MASK: usize = 0x0000_ffff_ffff_f000; // bits 12..48
    const MAIR_MASK: usize = 0b111 << 2;

    #[inline(always)]
    pub fn as_flags(&self) -> PteFlags {
        PteFlags::from_bits_truncate(self.0)
    }

    #[inline(always)]
    pub fn set_mair_idx(&mut self, idx: usize) {
        self.0 &= !Self::MAIR_MASK;
        self.0 |= idx << 2;
    }

    pub fn new_valid() -> Self {
        let flags = PteFlags::empty()
            | PteFlags::AF
            | PteFlags::VALID
            | PteFlags::NON_BLOCK
            | PteFlags::UXN;

        Self(flags.bits())
    }

    /// 创建空页表项
    pub const fn empty() -> Self {
        Self(0)
    }

    #[allow(unused)]
    pub fn update_flags<F>(&mut self, f: F)
    where
        F: FnOnce(&mut PteFlags),
    {
        let mut flags = self.as_flags();
        f(&mut flags);
        // 保留物理地址和 MAIR 索引，只更新标志位
        let preserved = self.0 & (Self::PHYS_ADDR_MASK | Self::MAIR_MASK);
        self.0 = preserved | flags.bits();
    }
}

impl PageTableEntry for Entry {
    fn valid(&self) -> bool {
        self.as_flags().contains(PteFlags::VALID)
    }

    fn paddr(&self) -> page_table_generic::PhysAddr {
        (self.0 & Self::PHYS_ADDR_MASK).into()
    }

    fn set_paddr(&mut self, paddr: page_table_generic::PhysAddr) {
        self.0 &= !Self::PHYS_ADDR_MASK;
        self.0 |= paddr.raw() & Self::PHYS_ADDR_MASK;
    }

    fn set_valid(&mut self, valid: bool) {
        if valid {
            self.0 |= (PteFlags::empty() | PteFlags::VALID).bits();
        } else {
            self.0 &= !(PteFlags::empty() | PteFlags::VALID).bits();
        }
    }

    fn is_huge(&self) -> bool {
        !self.as_flags().contains(PteFlags::NON_BLOCK)
    }

    fn set_is_huge(&mut self, b: bool) {
        let bits = (PteFlags::empty() | PteFlags::NON_BLOCK).bits();
        if b {
            self.0 &= !bits;
        } else {
            self.0 |= bits;
        }
    }

    fn set_mem_config(&mut self, config: page_table_generic::MemConfig) {
        use page_table_generic::{AccessFlags, MemAttributes};

        // 设置访问权限
        let writable = config.access.contains(AccessFlags::WRITE);
        let executable = config.access.contains(AccessFlags::EXECUTE);
        let user = config.access.contains(AccessFlags::LOWER);

        // AP[2:1] 访问权限位
        // AP_EL0 (bit 6) 和 AP_RO (bit 7)
        let ap_bits = if writable {
            if user {
                // AP = 01: 用户态可读写 (AP_EL0=1, AP_RO=0)
                PteFlags::AP_EL0.bits()
            } else {
                // AP = 00: 内核态可读写 (AP_EL0=0, AP_RO=0)
                0
            }
        } else if user {
            // AP = 11: 用户态只读 (AP_EL0=1, AP_RO=1)
            (PteFlags::AP_EL0 | PteFlags::AP_RO).bits()
        } else {
            // AP = 10: 内核态只读 (AP_EL0=0, AP_RO=1)
            PteFlags::AP_RO.bits()
        };

        // 清除旧的 AP 位并设置新的
        self.0 &= !(PteFlags::AP_EL0.bits() | PteFlags::AP_RO.bits());
        self.0 |= ap_bits;

        // UXN/PXN 执行权限位
        if !executable {
            self.0 |= (PteFlags::UXN | PteFlags::PXN).bits();
        } else {
            self.0 &= !(PteFlags::UXN | PteFlags::PXN).bits();
        }

        // 设置内存属性索引
        let attr_index = match config.attrs {
            MemAttributes::Normal => 0,   // AttrIndx = 0: Normal memory (cached)
            MemAttributes::Device => 1,   // AttrIndx = 1: Device memory
            MemAttributes::Uncached => 2, // AttrIndx = 2: Normal memory (non-cacheable)
        };

        self.set_mair_idx(attr_index);
    }

    fn mem_config(&self) -> page_table_generic::MemConfig {
        use page_table_generic::{AccessFlags, MemAttributes};

        let mut access = AccessFlags::READ;

        // 检查 AP 位确定写权限
        let ap = (self.0 >> 6) & 0x3;
        if ap == 0 || ap == 1 {
            // AP = 00 或 01 表示可写
            access |= AccessFlags::WRITE;
        }

        // 检查 UXN/PXN 位确定执行权限
        let no_exec = (self.0 & (PteFlags::UXN | PteFlags::PXN).bits()) != 0;
        if !no_exec {
            access |= AccessFlags::EXECUTE;
        }

        // 检查 AP_EL0 位确定是否为用户态
        if (ap & 0x1) != 0 {
            access |= AccessFlags::LOWER;
        }

        // 根据 AttrIndx 确定内存类型
        let attr_index = (self.0 >> 2) & 0x7;
        let attrs = match attr_index {
            0 => MemAttributes::Normal,   // Normal cached
            1 => MemAttributes::Device,   // Device
            2 => MemAttributes::Uncached, // Normal uncached
            _ => MemAttributes::Normal,   // 默认
        };

        page_table_generic::MemConfig { access, attrs }
    }
}

impl core::fmt::Debug for Entry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PTE {:?}", self.paddr())
    }
}

#[derive(Clone, Copy)]
pub struct Generic;

impl TableGeneric for Generic {
    type P = Entry;

    const PAGE_SIZE: usize = 0x1000;

    const LEVEL_BITS: &'static [usize] = &[9, 9, 9, 9];

    const MAX_BLOCK_LEVEL: usize = 3;

    fn flush(vaddr: Option<page_table_generic::VirtAddr>) {
        super::super::elx::flush_tlb(vaddr);
    }
}
