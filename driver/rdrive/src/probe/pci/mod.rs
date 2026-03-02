use core::{
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use ::pcie::*;
use alloc::{collections::btree_set::BTreeSet, vec::Vec};
use spin::{Mutex, Once};

pub use ::pcie::{Endpoint, PciCapability, PcieGeneric};
pub use rdif_pcie::{DriverGeneric, PciAddress, PciMem32, PciMem64, PcieController};

use crate::{
    Descriptor, Device, PlatformDevice, ProbeError, get_list,
    probe::OnProbeError,
    register::{DriverRegister, ProbeKind},
};

static PCIE: Once<Mutex<Vec<PcieEnumterator>>> = Once::new();

pub type FnOnProbe = fn(ep: &mut EndpointRc, plat_dev: PlatformDevice) -> Result<(), OnProbeError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Id {
    vendor: u16,
    device: u16,
}

pub fn new_driver_generic(mmio_base: NonNull<u8>) -> PcieController {
    PcieController::new(PcieGeneric::new(mmio_base))
}

fn pcie() -> &'static Mutex<Vec<PcieEnumterator>> {
    PCIE.call_once(|| {
        let ctrl_ls = get_list::<PcieController>();
        let mut vec = Vec::new();
        for ctrl in ctrl_ls.into_iter() {
            {
                let mut g = ctrl.lock().unwrap();
                g.open().unwrap();
            }

            vec.push(PcieEnumterator {
                ctrl,
                probed: BTreeSet::new(),
            });
        }
        Mutex::new(vec)
    })
}
pub(crate) fn probe_with(
    registers: &[DriverRegister],
    stop_if_fail: bool,
) -> Result<(), ProbeError> {
    let mut pcie_ls = pcie().lock();
    for ctrl in pcie_ls.iter_mut() {
        ctrl.probe(registers, stop_if_fail)?;
    }
    Ok(())
}

pub struct EndpointRc(Option<Endpoint>);

impl EndpointRc {
    fn new(ep: Endpoint) -> Self {
        Self(Some(ep))
    }

    pub fn take(&mut self) -> Endpoint {
        self.0.take().unwrap()
    }
}

impl Deref for EndpointRc {
    type Target = Endpoint;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl DerefMut for EndpointRc {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

struct PcieEnumterator {
    ctrl: Device<PcieController>,
    probed: BTreeSet<Id>,
}

impl PcieEnumterator {
    fn probe(
        &mut self,
        registers: &[DriverRegister],
        stop_if_fail: bool,
    ) -> Result<(), ProbeError> {
        let mut g = self.ctrl.lock().unwrap();

        for ep in enumerate_by_controller(&mut g, None) {
            debug!("PCIe endpiont: {}", ep);
            match self.probe_one(ep, registers, stop_if_fail) {
                Ok(_) => {} // Successfully probed, move to the next
                Err(e) => {
                    if stop_if_fail {
                        return Err(e);
                    } else {
                        warn!("Probe failed: {e}");
                    }
                }
            }
        }

        Ok(())
    }

    fn probe_one(
        &mut self,
        endpoint: Endpoint,
        registers: &[DriverRegister],
        stop_if_fail: bool,
    ) -> Result<(), ProbeError> {
        let id = Id {
            vendor: endpoint.vendor_id(),
            device: endpoint.device_id(),
        };
        if self.probed.contains(&id) {
            return Ok(());
        }

        let mut endpoint = EndpointRc::new(endpoint);

        for register in registers {
            let Some(pci_probe) = register.probe_kinds.iter().find_map(|probe| {
                if let ProbeKind::Pci { on_probe } = probe {
                    Some(on_probe)
                } else {
                    None
                }
            }) else {
                continue;
            };
            let mut desc = Descriptor::new();
            desc.name = register.name;
            desc.irq_parent = self.ctrl.descriptor().irq_parent;

            let plat_dev = PlatformDevice::new(desc);
            match (pci_probe)(&mut endpoint, plat_dev) {
                Ok(_) => {
                    self.probed.insert(id);
                    return Ok(());
                }
                Err(e) => match e {
                    OnProbeError::NotMatch => continue,
                    e => {
                        if stop_if_fail {
                            return Err(ProbeError::from(e));
                        }
                        warn!("Probe failed: {e}");
                    }
                },
            }
        }

        Ok(())
    }
}
