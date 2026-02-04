pub use mmio_api::{Error, Mmio, MmioOp, PhysAddr};

pub trait KernelOp: MmioOp {}

struct EmptyKernelOp;

impl KernelOp for EmptyKernelOp {}

impl MmioOp for EmptyKernelOp {
    fn ioremap(&self, _addr: PhysAddr, _size: usize) -> Result<Mmio, Error> {
        unimplemented!()
    }

    fn iounmap(&self, _mmio: &Mmio) {
        unimplemented!()
    }
}

static mut KERNEL_OP: &'static dyn KernelOp = &EmptyKernelOp;

pub(crate) fn set_kernel_op(op: &'static dyn KernelOp) {
    mmio_api::init(op);
    unsafe {
        KERNEL_OP = op;
    }
}

// pub(crate) fn kernel() -> &'static dyn KernelOp {
//     unsafe { KERNEL_OP }
// }
