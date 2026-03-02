#![no_std]
#![no_main]
#![feature(used_with_arg)]

extern crate alloc;
extern crate bare_test;

#[bare_test::tests]
mod tests {
    use bare_test::{
        fdt_parser::PciSpace,
        globals::{global_val, PlatformInfoKind},
        mem::iomap,
        println,
    };
    use log::info;
    use pcie::{
        enumerate_by_controller, CommandRegister, PciMem32, PciMem64, PcieController, PcieGeneric,
    };

    #[test]
    fn test_iter() {
        let PlatformInfoKind::DeviceTree(fdt) = &global_val().platform_info;
        let fdt = fdt.get();

        let pcie = fdt
            .find_compatible(&["pci-host-ecam-generic"])
            .next()
            .unwrap()
            .into_pci()
            .unwrap();

        let mut pcie_regs = alloc::vec![];

        println!("test nvme");

        println!("pcie: {}", pcie.node.name);

        for reg in pcie.node.reg().unwrap() {
            println!("pcie reg: {:#x}", reg.address);
            pcie_regs.push(iomap((reg.address as usize).into(), reg.size.unwrap()));
        }

        let base_vaddr = pcie_regs[0];

        info!("Init PCIE @{base_vaddr:?}");

        let i = PcieGeneric::new(base_vaddr);
        let mut drv = PcieController::new(i);

        for range in pcie.ranges().unwrap() {
            info!("{range:?}");
            match range.space {
                PciSpace::Memory32 => {
                    drv.set_mem32(
                        PciMem32 {
                            address: range.cpu_address as _,
                            size: range.size as _,
                        },
                        range.prefetchable,
                    );
                }
                PciSpace::Memory64 => {
                    drv.set_mem64(
                        PciMem64 {
                            address: range.cpu_address as _,
                            size: range.size as _,
                        },
                        range.prefetchable,
                    );
                }
                _ => {}
            }
        }

        for mut ep in enumerate_by_controller(&mut drv, None) {
            println!("{}", ep);
            println!("  BARs:");
            for i in 0..6 {
                if let Some(bar) = ep.bar(i) {
                    println!("    BAR{}: {:x?}", i, bar);
                }
            }
            for cap in ep.capabilities() {
                println!("  {:?}", cap);
            }

            ep.update_command(|mut cmd| {
                cmd.insert(CommandRegister::MEMORY_ENABLE);
                cmd
            });
        }

        println!("test passed!");
    }
}
