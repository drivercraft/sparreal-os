#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum CpuOnStatus {
    Ok = 0,
    NotSupported = 1,
    AlreadyOn = 2,
    InvalidParameters = 3,
    Other = 4,
}

impl CpuOnStatus {
    pub fn is_started(self) -> bool {
        matches!(self, Self::Ok | Self::AlreadyOn)
    }
}

impl From<usize> for CpuOnStatus {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::Ok,
            1 => Self::NotSupported,
            2 => Self::AlreadyOn,
            3 => Self::InvalidParameters,
            _ => Self::Other,
        }
    }
}

pub fn cpu_count() -> usize {
    crate::hal::al::cpu::cpu_count()
}

pub fn current_cpu_id() -> usize {
    crate::hal::al::cpu::current_cpu_id()
}

pub fn cpu_on(cpu_idx: usize) -> CpuOnStatus {
    crate::hal::al::cpu::cpu_on(cpu_idx).into()
}
