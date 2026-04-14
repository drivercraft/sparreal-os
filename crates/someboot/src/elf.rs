// RELA 重定位结构 (参考 include/uapi/linux/elf.h)
#[repr(C)]
pub struct Rela {
    pub r_offset: u64, // 需要重定位的地址
    pub r_info: u64,   // 类型和符号索引
    pub r_addend: i64, // 加数值
}

impl Rela {
    #[inline]
    fn r_type_raw(&self) -> u32 {
        (self.r_info & 0xFFFFFFFF) as u32
    }
}

/// 应用 .rela.dyn 重定位
/// # Safety
/// 此函数操作裸指针，调用者必须确保传入的指针范围有效且指向合法的 RELA 重定位表。
pub unsafe fn apply_reloc(load_offset: i128, start: *mut u8, end: *const u8, r_type: u32) {
    let mut reloc = start as *mut Rela;
    let end = end as usize;

    while (reloc as usize) < end {
        let current = unsafe { &mut *reloc };
        if current.r_type_raw() == r_type {
            let addr = (current.r_offset as i128 + load_offset) as usize as *mut usize;
            let val = (current.r_addend as i128 + load_offset) as usize;
            unsafe { *addr = val };
        }
        reloc = unsafe { reloc.add(1) };
    }
}

/// 应用 .rela.dyn 重定位
/// # Safety
/// 此函数操作裸指针，调用者必须确保传入的指针范围有效且指向合法的 RELA 重定位表。
pub unsafe fn reset(r_type: u32) {
    unsafe extern "C" {
        fn __rela_dyn_begin();
        fn __rela_dyn_end();
    }
    let start = __rela_dyn_begin as *mut u8;
    let end = __rela_dyn_end as *const u8;

    let mut reloc = start as *mut Rela;
    let end = end as usize;
    while (reloc as usize) < end {
        let current = unsafe { &mut *reloc };
        if current.r_type_raw() == r_type {
            let addr = current.r_offset as usize as *mut usize;
            unsafe { addr.write_volatile(current.r_addend as u64 as usize) };
        }
        reloc = unsafe { reloc.add(1) };
    }
}
