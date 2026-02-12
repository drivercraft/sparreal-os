use acpi::AcpiTables;
use acpi::sdt::madt::{Madt, MadtEntry};
use arrayvec::ArrayVec;
use core::{ffi::c_void, ptr::NonNull};

pub(crate) mod earlycon;
mod handle;
pub mod power;
// pub mod ram;

use crate::mem::phys_to_virt;
pub(crate) use handle::AcpiHandle;

/// RSDP存储
static mut RSDP: usize = 0;

/// 设置RSDP地址
#[allow(unused)]
pub(crate) fn set_rsdp(addr: *const c_void) {
    unsafe {
        RSDP = addr as usize;
    }
}

#[allow(unused)]
/// 获取RSDP地址
fn rsdp() -> Option<NonNull<u8>> {
    let rsdp = unsafe { RSDP };
    if rsdp == 0 {
        return None;
    }
    let ptr = phys_to_virt(rsdp);

    NonNull::new(ptr)
}

pub fn tables() -> Result<AcpiTables<AcpiHandle>, acpi::AcpiError> {
    unsafe {
        let rsdp = if RSDP == 0 {
            return Err(acpi::AcpiError::NoValidRsdp);
        } else {
            RSDP
        };

        let h = AcpiHandle;
        ::acpi::AcpiTables::from_rsdp(h, rsdp)
    }
}

pub fn cpu_id_list() -> Option<impl Iterator<Item = usize>> {
    let tables = tables().ok()?;
    let madt = tables.find_table::<Madt>()?;
    let madt = madt.get();

    // 仅用于枚举 CPU ID，避免引入动态分配。
    // 若平台 CPU 数量超过容量，将静默截断。
    let mut ids = ArrayVec::<usize, 256>::new();

    for entry in madt.entries() {
        match entry {
            // x86 (APIC/x2APIC)
            MadtEntry::LocalApic(e) if (e.flags & 1) != 0 => {
                let _ = ids.try_push(e.apic_id as usize);
            }
            MadtEntry::LocalX2Apic(e) if (e.flags & 1) != 0 => {
                let _ = ids.try_push(e.x2apic_id as usize);
            }

            // ARM (GIC) - MPIDR 是 PSCI/启动次核常用的硬件 ID
            MadtEntry::Gicc(e) if (e.flags & 1) != 0 => {
                let _ = ids.try_push(e.mpidr as usize);
            }

            _ => {}
        }
    }

    if ids.is_empty() {
        return None;
    }

    Some(ids.into_iter())
}
