#![no_std]
#![no_main]
#![feature(used_with_arg)]

extern crate alloc;
extern crate bare_test;

#[bare_test::tests]
mod tests {
    use alloc::{
        alloc::{alloc_zeroed, dealloc},
        vec,
    };
    use bare_test::{
        hal::al::{PhysAddr, VirtAddr},
        os::{
            mem::{ioremap, page_size},
            platform::{get_platform_descriptor, PlatformDescriptor},
        },
        *,
    };
    use core::{alloc::Layout, num::NonZeroUsize, ptr::NonNull};
    use dma_api::{DeviceDma, DmaDirection, DmaHandle, DmaMapHandle, DmaOp};
    use fdt_parser::{Fdt, Node, PciSpace};
    use nvme_driver::{Config, Nvme};
    use pcie::{
        enumerate_by_controller, CommandRegister, DeviceType, PciMem32, PciMem64, PcieController,
        PcieGeneric,
    };

    static DMA_OP: BareTestDma = BareTestDma;

    struct BareTestDma;

    impl DmaOp for BareTestDma {
        fn page_size(&self) -> usize {
            page_size()
        }

        unsafe fn map_single(
            &self,
            dma_mask: u64,
            addr: NonNull<u8>,
            size: NonZeroUsize,
            align: usize,
            _direction: DmaDirection,
        ) -> core::result::Result<DmaMapHandle, dma_api::DmaError> {
            let layout = Layout::from_size_align(size.get(), align.max(1))?;
            let phys: PhysAddr = VirtAddr::from(addr).into();
            let dma_addr = phys.raw() as u64;

            if dma_addr > dma_mask || !dma_addr.is_multiple_of(align.max(1) as u64) {
                return Err(dma_api::DmaError::AlignMismatch {
                    required: align.max(1),
                    address: dma_addr.into(),
                });
            }

            Ok(unsafe { DmaMapHandle::new(addr, dma_addr.into(), layout, None) })
        }

        unsafe fn unmap_single(&self, _handle: DmaMapHandle) {}

        unsafe fn alloc_coherent(&self, dma_mask: u64, layout: Layout) -> Option<DmaHandle> {
            let ptr = unsafe { alloc_zeroed(layout) };
            let ptr = NonNull::new(ptr)?;
            let phys: PhysAddr = VirtAddr::from(ptr).into();
            let dma_addr = phys.raw() as u64;

            if dma_addr > dma_mask || !dma_addr.is_multiple_of(layout.align() as u64) {
                unsafe { dealloc(ptr.as_ptr(), layout) };
                return None;
            }

            Some(unsafe { DmaHandle::new(ptr, dma_addr.into(), layout) })
        }

        unsafe fn dealloc_coherent(&self, handle: DmaHandle) {
            unsafe { dealloc(handle.as_ptr().as_ptr(), handle.layout()) };
        }
    }

    #[test]
    fn test_framework_boot() {
        println!("nvme bare-test bootstrap ok");
    }

    #[test]
    #[timeout = 100]
    fn test_framework_timeout_path() {
        println!("nvme bare-test timeout guard ok");
    }

    #[test]
    #[timeout = 10000]
    fn test_nvme_end_to_end() {
        println!("nvme discovery start");

        let mut nvme = get_nvme();

        println!("nvme init ok");

        let namespace_list = nvme.namespace_list().unwrap();

        println!("namespace count: {}", namespace_list.len());
        assert!(!namespace_list.is_empty(), "namespace list is empty");

        for ns in &namespace_list {
            println!(
                "namespace id={} lba_size={} lba_count={}",
                ns.id, ns.lba_size, ns.lba_count
            );
        }

        println!("namespace query ok");

        let ns = namespace_list[0];

        for block in 0..128 {
            let mut write_buf = vec![0u8; ns.lba_size];
            let message = alloc::format!("hello world! block {block}");
            let message_bytes = message.as_bytes();

            write_buf[..message_bytes.len()].copy_from_slice(message_bytes);

            nvme.block_write_sync(&ns, block, &write_buf).unwrap();

            let mut read_buf = vec![0u8; ns.lba_size];
            nvme.block_read_sync(&ns, block, &mut read_buf).unwrap();

            assert_eq!(&read_buf[..message_bytes.len()], message_bytes);

            if block == 0 || block == 127 {
                println!("block {} io ok", block);
            }
        }

        println!("nvme io ok");
    }

    fn get_nvme() -> Nvme {
        let PlatformDescriptor::DeviceTree(dtb) = get_platform_descriptor() else {
            panic!("device tree not found");
        };
        let fdt = Fdt::from_bytes(dtb.as_slice()).unwrap();
        let pcie = match fdt
            .find_compatible(&["pci-host-ecam-generic"])
            .into_iter()
            .next()
            .unwrap()
        {
            Node::Pci(pci) => pci,
            _ => panic!("pci host bridge not found"),
        };

        println!("pcie: {}", pcie.name());

        let mut pcie_regs = vec![];

        for reg in pcie.reg().unwrap() {
            println!("pcie reg: {:#x}", reg.address);
            let reg_size = reg.size.expect("pcie reg size missing");
            pcie_regs.push(ioremap((reg.address as usize).into(), reg_size).unwrap());
        }

        let base_vaddr = pcie_regs[0];
        let base_vaddr = NonNull::new(base_vaddr.raw() as *mut u8).unwrap();

        let mut controller = PcieController::new(PcieGeneric::new(base_vaddr));

        for range in pcie.ranges().unwrap() {
            match range.space {
                PciSpace::Memory32 => {
                    controller.set_mem32(
                        PciMem32 {
                            address: range.cpu_address as _,
                            size: range.size as _,
                        },
                        range.prefetchable,
                    );
                }
                PciSpace::Memory64 => {
                    controller.set_mem64(
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

        let page_size = page_size();

        for mut ep in enumerate_by_controller(&mut controller, None) {
            println!("{}", ep);
            if ep.device_type() == DeviceType::NvmeController {
                let bar = ep.bar_mmio(0).unwrap();
                println!("bar0: [{:#x}, {:#x})", bar.start, bar.end);
                println!("nvme discovery ok");

                let addr = ioremap(bar.start.into(), bar.count()).unwrap();
                let addr = NonNull::new(addr.raw() as *mut u8).unwrap();

                ep.update_command(|mut cmd| {
                    cmd.insert(CommandRegister::BUS_MASTER_ENABLE | CommandRegister::MEMORY_ENABLE);
                    cmd
                });

                return Nvme::new(
                    addr,
                    DeviceDma::new(u64::MAX, &DMA_OP),
                    Config {
                        page_size,
                        io_queue_pair_count: 1,
                    },
                )
                .unwrap();
            }
        }

        panic!("no nvme found");
    }
}
