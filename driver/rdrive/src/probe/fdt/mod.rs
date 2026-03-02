use alloc::{
    collections::{BTreeMap, btree_set::BTreeSet},
    vec::Vec,
};
use core::ptr::NonNull;
use spin::{Mutex, Once};

pub use fdt_parser::*;

use crate::{
    Descriptor, DeviceId, PlatformDevice,
    error::DriverError,
    probe::OnProbeError,
    register::{DriverRegister, ProbeKind},
};

use super::ProbeError;

static SYSTEM: Once<System> = Once::new();

pub fn init(fdt_addr: NonNull<u8>) -> Result<(), DriverError> {
    let sys = System::new(fdt_addr)?;
    SYSTEM.call_once(|| sys);
    Ok(())
}

pub fn probe_register(
    register: &DriverRegister,
) -> Result<Vec<Result<(), OnProbeError>>, ProbeError> {
    let sys = system();
    sys.probe_register(register)
}

pub(crate) fn system() -> &'static System {
    SYSTEM.get().expect("rdrive not init")
}

#[derive(Clone)]
pub struct FdtInfo<'a> {
    pub node: Node<'a>,
    phandle_2_device_id: BTreeMap<Phandle, DeviceId>,
}

impl<'a> FdtInfo<'a> {
    pub fn phandle_to_device_id(&self, phandle: Phandle) -> Option<DeviceId> {
        self.phandle_2_device_id.get(&phandle).copied()
    }

    pub fn find_clk_by_name(&'a self, name: &str) -> Option<ClockRef<'a>> {
        self.node.clocks().find(|clock| clock.name == Some(name))
    }

    pub fn interrupts(&self) -> Vec<Vec<u32>> {
        let mut out = Vec::new();
        if let Some(raws) = self.node.interrupts() {
            for raw in raws {
                out.push(raw.collect());
            }
        }
        out
    }
}

pub type FnOnProbe = fn(fdt: FdtInfo<'_>, plat_dev: PlatformDevice) -> Result<(), OnProbeError>;

pub struct System {
    phandle_2_device_id: BTreeMap<Phandle, DeviceId>,
    fdt_addr: usize,
    // keep unique by driver register name in FDT mode
    probed_names: Mutex<BTreeSet<&'static str>>,
}

unsafe impl Send for System {}

impl System {
    pub fn fdt_addr(&self) -> NonNull<u8> {
        unsafe { NonNull::new_unchecked(self.fdt_addr as *mut u8) }
    }

    pub fn phandle_to_device_id(&self, phandle: Phandle) -> Option<DeviceId> {
        self.phandle_2_device_id.get(&phandle).copied()
    }
}

impl System {
    pub fn new(fdt_addr: NonNull<u8>) -> Result<Self, DriverError> {
        let fdt = Fdt::from_ptr(fdt_addr)?;
        let mut phandle_2_device_id = BTreeMap::new();
        for node in fdt.all_nodes() {
            if let Some(phandle) = node.phandle() {
                phandle_2_device_id.insert(phandle, DeviceId::new());
            }
        }
        Ok(Self {
            phandle_2_device_id,
            fdt_addr: fdt_addr.as_ptr() as usize,
            probed_names: Mutex::new(BTreeSet::new()),
        })
    }

    fn new_device_id(&self, phandle: Option<Phandle>) -> DeviceId {
        if let Some(phandle) = phandle {
            self.phandle_2_device_id[&phandle]
        } else {
            DeviceId::new()
        }
    }

    fn get_fdt_match_nodes(
        &self,
        register: &DriverRegister,
        fdt: &Fdt<'static>,
    ) -> Vec<ProbeFdtInfo> {
        let mut out = Vec::new();
        for node in fdt.all_nodes() {
            if matches!(node.status(), Some(Status::Disabled)) {
                continue;
            }

            let node_compatibles = node.compatibles().collect::<Vec<_>>();

            for probe in register.probe_kinds {
                let &ProbeKind::Fdt {
                    compatibles,
                    on_probe,
                } = probe
                else {
                    continue;
                };

                for campatible in &node_compatibles {
                    if compatibles.contains(campatible) {
                        out.push(ProbeFdtInfo {
                            name: register.name,
                            node: node.clone(),
                            on_probe,
                        });
                    }
                }
            }
        }
        out
    }

    fn probe_register(
        &self,
        register: &DriverRegister,
    ) -> Result<Vec<Result<(), OnProbeError>>, ProbeError> {
        let fdt: Fdt<'static> = Fdt::from_ptr(self.fdt_addr())?;
        let node_ls = self.get_fdt_match_nodes(register, &fdt);
        let mut out = Vec::new();
        for node_info in node_ls {
            if self.probed_names.lock().contains(node_info.name) {
                // skip duplicated register name in FDT system
                continue;
            }
            let id = self.new_device_id(node_info.node.phandle());

            let irq_parent = node_info
                .node
                .interrupt_parent()
                .filter(|p| p.node.phandle() != node_info.node.phandle())
                .and_then(|n| n.node.phandle())
                .and_then(|p| self.phandle_2_device_id.get(&p).copied());

            let phandle_map = self.phandle_2_device_id.clone();

            debug!("Probe [{}]->[{}]", node_info.node.name, node_info.name);

            let descriptor = Descriptor {
                name: node_info.name,
                device_id: id,
                irq_parent,
            };

            let res = (node_info.on_probe)(
                FdtInfo {
                    node: node_info.node.clone(),
                    phandle_2_device_id: phandle_map,
                },
                PlatformDevice::new(descriptor),
            );

            if res.is_ok() {
                self.probed_names.lock().insert(node_info.name);
            }

            out.push(res);
        }

        Ok(out)
    }
}

struct ProbeFdtInfo {
    name: &'static str,
    node: Node<'static>,
    on_probe: FnOnProbe,
}
