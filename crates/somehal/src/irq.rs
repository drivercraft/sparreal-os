use crate::ArchTrait;

pub fn register_timer_handler(handler: fn()) {
    crate::arch::Arch::register_timer_handler(handler);
}
