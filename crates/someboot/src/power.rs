use crate::{
    ArchTrait, DCacheOp,
    arch::_secondary_entry,
    mem::{__kimage_va, dcache_range, virt_to_phys},
};

pub fn shutdown() -> ! {
    crate::arch::Arch::shutdown()
}

pub fn cpu_on(cpu_idx: usize) -> Result<(), CpuOnError> {
    let entry = secondary_entry_addr();
    let arg = crate::smp::cpu_meta_addr(cpu_idx).ok_or(CpuOnError::InvalidParameters)?;
    let cpu_id = crate::smp::cpu_idx_to_id(cpu_idx).ok_or(CpuOnError::InvalidParameters)?;
    debug!("Power on CPU {cpu_idx:#x} (hard {cpu_id:#x}) at entry {entry:#x}, arg {arg:#x}");
    let kimg = crate::mem::kimage_range();
    let kimg_start = __kimage_va(kimg.start);
    let size = kimg.end - kimg.start;
    dcache_range(DCacheOp::Clean, kimg_start, size);

    crate::arch::Arch::cpu_on(cpu_id, entry, arg)?;
    Ok(())
}

/// secondary entry address
/// arg0 is stack top
fn secondary_entry_addr() -> usize {
    let ptr = _secondary_entry as *const u8;
    virt_to_phys(ptr)
}

#[derive(thiserror::Error, Debug)]
pub enum CpuOnError {
    #[error("CPU on is not supported")]
    NotSupported,
    #[error("CPU is already on")]
    AlreadyOn,
    #[error("Invalid parameters")]
    InvalidParameters,
    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}
