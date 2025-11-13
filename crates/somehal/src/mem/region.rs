#[derive(Debug, Clone, Copy)]
pub struct MemoryDescriptor {
    pub name: &'static str,
    pub physical_start: usize,
    pub size_in_bytes: usize,
    pub memory_type: MemoryType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    Usable,
    Reserved,
}
