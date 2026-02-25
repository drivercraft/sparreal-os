pub use rdif_intc;
pub use someboot::irq::*;

use crate::{arch::Plat, common::PlatOp};

pub fn irq_set_enable(irq: rdrive::IrqId, enable: bool) {
    debug!("Setting IRQ {:?} enable to {}", irq, enable);
    Plat::irq_set_enable(irq, enable);
}

pub fn systick_irq() -> rdrive::IrqId {
    Plat::systick_irq()
}

pub(crate) fn _handle_irq(hwirq: IrqId) {
    unsafe extern "Rust" {
        fn _someboot_handle_irq(hwirq: IrqId);
    }
    unsafe {
        _someboot_handle_irq(hwirq);
    }
}

pub fn irq_handler_raw() -> IrqId {
    Plat::irq_handler()
}
