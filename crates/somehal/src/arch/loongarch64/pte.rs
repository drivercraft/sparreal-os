//! LoongArch64 页表项 (Page Table Entry)
//!
//! 使用 tock-registers 风格定义页表项，提供类型安全的寄存器访问
//! 参考: LoongArch64 参考手册 Vol. 1 - 5.4.2 节

use page_table_generic::{MemAttributes, PageTableEntry};
use tock_registers::interfaces::*;
use tock_registers::register_bitfields;
use tock_registers::registers::*;

// LoongArch64 页表项寄存器位域定义
register_bitfields![u64,
    /// LoongArch64 单页页表项 (Page Table Entry)
    ///
    /// 布局参考 LoongArch64 参考手册 5.4.2 节
    PTE [
        /// V - 有效位 (bit 0)
        VALID OFFSET(0) NUMBITS(1) [],

        /// D - 脏位 (bit 1)
        DIRTY OFFSET(1) NUMBITS(1) [],

        /// PLV - 特权级 (bits 2-3)
        PLV OFFSET(2) NUMBITS(2) [
            PLV0 = 0b00,  // 内核态
            PLV1 = 0b01,  // 特权级1
            PLV2 = 0b10,  // 特权级2
            PLV3 = 0b11   // 用户态
        ],

        /// 缓存属性 (bits 4-5)
        CACHE OFFSET(4) NUMBITS(2) [
            SUC = 0b00,  // 强序非缓存 (Strongly-ordered UnCached)
            CC  = 0b01,  // 一致性缓存 (Coherent Cached)
            WUC = 0b10   // 弱序非缓存 (Weakly-ordered UnCached)
        ],

        /// 目录项大页表项标志位 H，为 1 表示此时的目录项实际上存放了一个大页的页表项信息；
        GH OFFSET(6) NUMBITS(1) [],

        /// P - 存在位 (bit 7)
        PRESENT OFFSET(7) NUMBITS(1) [],

        /// W - 写位 (bit 8)
        WRITE OFFSET(8) NUMBITS(1) [],

        /// M - 修改位 (bit 9)
        MODIFIED OFFSET(9) NUMBITS(1) [],

        /// PROTNONE (bit 10)
        PROTNONE OFFSET(10) NUMBITS(1) [],

        /// SPECIAL (bit 11)
        SPECIAL OFFSET(11) NUMBITS(1) [],

        /// HGLOBAL - 巨页全局位 (bit 12, PMD 用)
        HGLOBAL OFFSET(12) NUMBITS(1) [],

        /// 物理页帧号 (bits 12-51)
        /// 注意: 根据 PDF, PPN 占据 bits [51:12]
        PHYS_ADDR OFFSET(12) NUMBITS(40) [],

        /// NR - 禁止读位 (bit 61)
        NO_READ OFFSET(61) NUMBITS(1) [],

        /// NX - 禁止执行位 (bit 62)
        NO_EXEC OFFSET(62) NUMBITS(1) [],

        /// RPLV (bit 63)
        RPLV OFFSET(63) NUMBITS(1) [],
    ],
];

/// 页表项寄存器类型别名
type PteRegister = ReadWrite<u64, PTE::Register>;

/// LoongArch64 页表项
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Entry(u64);

impl Entry {
    /// 获取类型化的寄存器访问接口
    #[inline(always)]
    fn as_typed(&self) -> &PteRegister {
        unsafe { &*(self as *const Self as *const PteRegister) }
    }

    /// 获取可变类型化的寄存器访问接口
    #[inline(always)]
    fn as_typed_mut(&mut self) -> &mut PteRegister {
        unsafe { &mut *(self as *mut Self as *mut PteRegister) }
    }

    /// 创建空页表项
    pub const fn empty() -> Self {
        Self(0)
    }
}

impl PageTableEntry for Entry {
    fn new_valid() -> Self {
        let mut entry = Self::empty();
        entry.set_valid(true);
        entry
    }

    fn valid(&self) -> bool {
        self.as_typed().is_set(PTE::VALID)
    }

    fn paddr(&self) -> page_table_generic::PhysAddr {
        (self.as_typed().read(PTE::PHYS_ADDR) << 12).into()
    }

    fn set_paddr(&mut self, paddr: page_table_generic::PhysAddr) {
        self.as_typed_mut()
            .modify(PTE::PHYS_ADDR.val(paddr.raw() as u64 >> 12));
    }

    fn set_valid(&mut self, valid: bool) {
        self.as_typed_mut().modify(if valid {
            PTE::VALID::SET + PTE::PRESENT::SET
        } else {
            PTE::VALID::CLEAR + PTE::PRESENT::CLEAR
        });
    }

    fn is_huge(&self) -> bool {
        self.as_typed().is_set(PTE::GH)
    }

    fn set_is_huge(&mut self, b: bool) {
        self.as_typed_mut()
            .modify(if b { PTE::GH::SET } else { PTE::GH::CLEAR });
    }

    fn is_writable(&self) -> bool {
        self.as_typed().is_set(PTE::WRITE)
    }

    fn set_writable(&mut self, b: bool) {
        self.as_typed_mut().modify(if b {
            PTE::WRITE::SET
        } else {
            PTE::WRITE::CLEAR
        });
    }

    fn is_executable(&self) -> bool {
        !self.as_typed().is_set(PTE::NO_EXEC)
    }

    fn set_executable(&mut self, b: bool) {
        self.as_typed_mut().modify(if b {
            PTE::NO_EXEC::CLEAR
        } else {
            PTE::NO_EXEC::SET
        });
    }

    fn is_lower_access(&self) -> bool {
        matches!(
            self.as_typed().read_as_enum(PTE::PLV),
            Some(PTE::PLV::Value::PLV3)
        )
    }

    fn set_lower_access(&mut self, b: bool) {
        let plv = if b { PTE::PLV::PLV3 } else { PTE::PLV::PLV0 }; // PLV3 或 PLV0
        self.as_typed_mut().modify(plv);
    }

    fn is_global(&self) -> bool {
        self.as_typed().is_set(PTE::GH)
    }

    fn set_global(&mut self, b: bool) {
        self.as_typed_mut()
            .modify(if b { PTE::GH::SET } else { PTE::GH::CLEAR });
    }

    fn is_accessed(&self) -> bool {
        // LoongArch64 无硬件 accessed 位
        false
    }

    fn set_accessed(&mut self, _b: bool) {
        // LoongArch64 不支持软件 accessed 位
    }

    fn is_dirty(&self) -> bool {
        self.as_typed().is_set(PTE::DIRTY)
    }

    fn set_dirty(&mut self, b: bool) {
        self.as_typed_mut().modify(if b {
            PTE::DIRTY::SET
        } else {
            PTE::DIRTY::CLEAR
        });
    }

    fn mem_attr(&self) -> MemAttributes {
        match self.as_typed().read_as_enum(PTE::CACHE) {
            Some(PTE::CACHE::Value::SUC) => MemAttributes::Device,
            Some(PTE::CACHE::Value::CC) => MemAttributes::Normal,
            Some(PTE::CACHE::Value::WUC) => MemAttributes::Uncached,
            _ => MemAttributes::Normal,
        }
    }

    fn set_mem_attr(&mut self, attr: MemAttributes) {
        let cache = match attr {
            MemAttributes::Device => 0b00,                         // SUC
            MemAttributes::Normal | MemAttributes::PerCpu => 0b01, // CC
            MemAttributes::Uncached => 0b10,                       // WUC
        };
        self.as_typed_mut().modify(PTE::CACHE.val(cache));
    }
}

impl core::fmt::Debug for Entry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Entry")
            .field("raw", &format_args!("{:#018x}", self.0))
            .field("valid", &self.valid())
            .field("huge", &self.is_huge())
            .field("global", &self.is_global())
            .field("writable", &self.is_writable())
            .field("dirty", &self.is_dirty())
            .field("exec", &self.is_executable())
            .field("lower", &self.is_lower_access())
            .field("paddr", &format_args!("{:#x}", self.paddr()))
            .finish()
    }
}
