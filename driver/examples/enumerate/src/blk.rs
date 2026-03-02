extern crate alloc;

use log::debug;
use rdif_intc::Intc;
use rdrive::probe::OnProbeError;
use rdrive::register::{DriverRegister, ProbeKind, ProbeLevel, ProbePriority};

use rdrive::{PlatformDevice, register::FdtInfo};

pub fn register() -> DriverRegister {
    DriverRegister {
        name: "Virtio",
        probe_kinds: &[ProbeKind::Fdt {
            compatibles: &["virtio,mmio"],
            on_probe: probe,
        }],
        level: ProbeLevel::PostKernel,
        priority: ProbePriority::DEFAULT,
    }
}

fn probe(info: FdtInfo<'_>, _dev: PlatformDevice) -> Result<(), OnProbeError> {
    let mut reg = info.node.reg().ok_or(OnProbeError::other(format!(
        "[{}] has no reg",
        info.node.name()
    )))?;

    if let Some(irq) = _dev.descriptor.irq_parent {
        let intc = rdrive::get::<Intc>(irq).unwrap();

        if let Some(irqs) = info.node.interrupts() {
            for irq_prop in irqs {
                let irq_prop: Vec<u32> = irq_prop.collect();
                let irq_id = intc.lock().unwrap().setup_irq_by_fdt(&irq_prop);
                debug!(
                    "virtio mmio device [{}] setup irq: {:?}",
                    info.node.name(),
                    irq_id
                );
            }
        }

        println!("parent intc: {:?}", intc.descriptor().name);
    }

    let base_reg = reg.next().unwrap();
    let mmio_size = base_reg.size.unwrap_or(0x1000);

    debug!(
        "virtio block device MMIO base address: {:#x}, size: {}",
        base_reg.address, mmio_size
    );

    Err(OnProbeError::NotMatch)
}
