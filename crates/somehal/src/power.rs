use crate::ArchTrait;

pub fn shutdown() -> ! {
    crate::arch::Arch::shutdown()
}
