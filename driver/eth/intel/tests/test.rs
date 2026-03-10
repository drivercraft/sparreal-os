#![no_std]
#![no_main]
#![feature(used_with_arg)]

extern crate alloc;
extern crate bare_test;

#[bare_test::tests]
mod tests {
    use alloc::{boxed::Box, vec, vec::Vec};
    use core::{num::NonZeroUsize, ptr::NonNull, time::Duration};

    use dma_api::{DmaDirection, DmaMapHandle, DmaOp};

    use bare_test::{
        os::{
            mem::{dma::kernel_dma_op, mmio::kernel_mmio_op},
            platform::{PlatformDescriptor, get_platform_descriptor},
        },
        *,
    };
    use eth_intel::E1000;
    use fdt_edit::{Fdt, NodeType, PciSpace};
    use pcie::{
        CommandRegister, PciMem32, PciMem64, PcieController, PcieGeneric, enumerate_by_controller,
    };
    use rdif_net::{Buffer, IRxQueue, ITxQueue, Interface, NetError};
    use smoltcp::{
        iface::{Config, Interface as SmolInterface, SocketSet},
        phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
        socket::icmp::{self, Socket as IcmpSocket},
        time::Instant,
        wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address},
    };

    const LOCAL_IP: IpAddress = IpAddress::v4(10, 0, 2, 15);
    const GATEWAY_IP: Ipv4Address = Ipv4Address::new(10, 0, 2, 2);

    fn now() -> Instant {
        let ms = os::time::since_boot().as_millis() as i64;
        Instant::from_millis(ms)
    }

    fn spin_delay(duration: Duration) {
        let start = os::time::since_boot();
        while os::time::since_boot().saturating_sub(start) < duration {
            core::hint::spin_loop();
        }
    }

    struct RxSlot {
        storage: Vec<u8>,
        map: DmaMapHandle,
        req_id: Option<rdif_net::RequestId>,
    }

    struct E1000Device {
        tx: Box<dyn ITxQueue>,
        rx: Box<dyn IRxQueue>,
        slots: Vec<RxSlot>,
    }

    impl E1000Device {
        fn new(tx: Box<dyn ITxQueue>, rx: Box<dyn IRxQueue>) -> Self {
            let cfg = rx.buff_config();
            let mut slots = Vec::new();

            for _ in 0..64 {
                let mut storage = vec![0u8; cfg.size.max(1536)];
                let map = unsafe {
                    kernel_dma_op()
                        .map_single(
                            cfg.dma_mask,
                            NonNull::new(storage.as_mut_ptr()).expect("nonnull rx buffer"),
                            NonZeroUsize::new(storage.len()).expect("nonzero rx buffer"),
                            cfg.align.max(1),
                            DmaDirection::FromDevice,
                        )
                        .expect("map rx buffer")
                };

                slots.push(RxSlot {
                    storage,
                    map,
                    req_id: None,
                });
            }

            let mut dev = Self { tx, rx, slots };
            for idx in 0..dev.slots.len() {
                let _ = dev.refill_slot(idx);
            }
            dev
        }

        fn refill_slot(&mut self, idx: usize) -> core::result::Result<(), NetError> {
            let slot = &mut self.slots[idx];
            if slot.req_id.is_some() {
                return Ok(());
            }

            let req_id = self.rx.submit_request(rdif_net::RxRequest {
                buffer: Buffer {
                    virt: slot.storage.as_mut_ptr(),
                    bus: slot.map.dma_addr().as_u64(),
                    size: slot.storage.len(),
                },
            })?;

            slot.req_id = Some(req_id);
            Ok(())
        }

        fn poll_rx_packet(&mut self) -> Option<Vec<u8>> {
            for idx in 0..self.slots.len() {
                let req_id = self.slots[idx].req_id?;
                match self.rx.poll_request(req_id) {
                    Ok(resp) => {
                        let len = resp.len.min(self.slots[idx].storage.len());
                        let packet = self.slots[idx].storage[..len].to_vec();
                        self.slots[idx].req_id = None;
                        let _ = self.refill_slot(idx);
                        return Some(packet);
                    }
                    Err(NetError::Retry) => {}
                    Err(_) => {
                        self.slots[idx].req_id = None;
                        let _ = self.refill_slot(idx);
                    }
                }
            }

            None
        }
    }

    impl Drop for E1000Device {
        fn drop(&mut self) {
            for slot in &self.slots {
                unsafe {
                    kernel_dma_op().unmap_single(slot.map);
                }
            }
        }
    }

    struct E1000RxToken {
        data: Vec<u8>,
    }

    impl RxToken for E1000RxToken {
        fn consume<R, F>(self, f: F) -> R
        where
            F: FnOnce(&[u8]) -> R,
        {
            f(&self.data)
        }
    }

    struct E1000TxToken<'a> {
        tx: &'a mut dyn ITxQueue,
    }

    impl<'a> TxToken for E1000TxToken<'a> {
        fn consume<R, F>(self, len: usize, f: F) -> R
        where
            F: FnOnce(&mut [u8]) -> R,
        {
            let mut buffer = vec![0u8; len];
            let ret = f(&mut buffer);

            let req_id = loop {
                match self
                    .tx
                    .submit_request(rdif_net::TxRequest { data: &buffer })
                {
                    Ok(req_id) => break req_id,
                    Err(NetError::Retry) => spin_delay(Duration::from_millis(1)),
                    Err(e) => panic!("tx submit failed: {e:?}"),
                }
            };

            loop {
                match self.tx.poll_request(req_id) {
                    Ok(()) => break,
                    Err(NetError::Retry) => spin_delay(Duration::from_millis(1)),
                    Err(e) => panic!("tx poll failed: {e:?}"),
                }
            }

            ret
        }
    }

    impl Device for E1000Device {
        type RxToken<'a>
            = E1000RxToken
        where
            Self: 'a;
        type TxToken<'a>
            = E1000TxToken<'a>
        where
            Self: 'a;

        fn receive(
            &mut self,
            _timestamp: Instant,
        ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
            let data = self.poll_rx_packet()?;
            Some((
                E1000RxToken { data },
                E1000TxToken {
                    tx: self.tx.as_mut(),
                },
            ))
        }

        fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
            Some(E1000TxToken {
                tx: self.tx.as_mut(),
            })
        }

        fn capabilities(&self) -> DeviceCapabilities {
            let mut caps = DeviceCapabilities::default();
            caps.max_transmission_unit = self.tx.mtu();
            caps.medium = Medium::Ethernet;
            caps.max_burst_size = Some(1);
            caps
        }
    }

    #[test]
    #[timeout = 10000]
    fn ping_test() {
        println!("ping_test: e1000 discovery start");

        let mut nic = get_e1000().expect("no e1000 found on pci bus");
        let mac = nic.mac_address();
        println!(
            "e1000 mac: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        );

        let tx = nic.create_tx_queue().expect("create tx queue");
        let rx = nic.create_rx_queue().expect("create rx queue");
        let mut dev = E1000Device::new(tx, rx);

        let config = Config::new(HardwareAddress::Ethernet(EthernetAddress::from_bytes(&mac)));
        let mut iface = SmolInterface::new(config, &mut dev, now());
        iface.update_ip_addrs(|addrs| {
            addrs.push(IpCidr::new(LOCAL_IP, 24)).unwrap();
        });
        iface
            .routes_mut()
            .add_default_ipv4_route(GATEWAY_IP)
            .unwrap();

        let rx_buf = icmp::PacketBuffer::new(vec![icmp::PacketMetadata::EMPTY], vec![0; 512]);
        let tx_buf = icmp::PacketBuffer::new(vec![icmp::PacketMetadata::EMPTY], vec![0; 512]);
        let icmp_socket = IcmpSocket::new(rx_buf, tx_buf);

        let mut sockets = SocketSet::new(vec![]);
        let icmp_handle = sockets.add(icmp_socket);

        let target = IpAddress::Ipv4(GATEWAY_IP);
        let ident = 0x22b;
        let mut sent = false;
        let mut received = false;

        for seq in 0u16..300 {
            let _ = iface.poll(now(), &mut dev, &mut sockets);

            let socket = sockets.get_mut::<IcmpSocket>(icmp_handle);
            if !socket.is_open() {
                socket.bind(icmp::Endpoint::Ident(ident)).unwrap();
            }

            if !sent && socket.can_send() {
                let repr = smoltcp::wire::Icmpv4Repr::EchoRequest {
                    ident,
                    seq_no: seq,
                    data: b"sparreal ping",
                };
                let payload = socket.send(repr.buffer_len(), target).unwrap();
                let mut packet = smoltcp::wire::Icmpv4Packet::new_unchecked(payload);
                repr.emit(&mut packet, &dev.capabilities().checksum);
                sent = true;
                println!("ping_test: icmp echo request sent");
            }

            if sent && socket.can_recv() {
                match socket.recv() {
                    Ok((_data, addr)) => {
                        println!("ping_test: icmp echo reply from {addr:?}");
                        received = true;
                        break;
                    }
                    Err(_) => {}
                }
            }

            spin_delay(Duration::from_millis(10));
        }

        assert!(received, "ping_test: no icmp echo reply received");
        println!("ping_test: completed");
    }

    fn get_e1000() -> Option<E1000> {
        let PlatformDescriptor::DeviceTree(dtb) = get_platform_descriptor() else {
            panic!("device tree not found");
        };

        let fdt = Fdt::from_bytes(dtb.as_slice()).unwrap();
        let (pcie_name, pcie) = match fdt
            .find_compatible(&["pci-host-ecam-generic"])
            .into_iter()
            .next()
            .unwrap()
        {
            node @ NodeType::Pci(_) => {
                let name = node.name();
                match node {
                    NodeType::Pci(pci) => (name, pci),
                    _ => unreachable!(),
                }
            }
            _ => panic!("pci host bridge not found"),
        };

        println!("pcie: {}", pcie_name);

        let reg = pcie.regs().into_iter().next().expect("pcie reg missing");
        let reg_size = reg.size.expect("pcie reg size missing");

        let mut controller = PcieController::new(
            PcieGeneric::new(reg.address as usize, reg_size as usize, kernel_mmio_op()).unwrap(),
        );

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

        for mut ep in enumerate_by_controller(&mut controller, None) {
            println!("{}", ep);
            if E1000::check_vid_did(ep.vendor_id(), ep.device_id()) {
                let bar = ep.bar_mmio(0).expect("bar0");
                ep.update_command(|mut cmd| {
                    cmd.insert(CommandRegister::BUS_MASTER_ENABLE | CommandRegister::MEMORY_ENABLE);
                    cmd
                });

                return Some(
                    E1000::new(
                        bar.start as u64,
                        bar.count(),
                        u64::MAX,
                        kernel_dma_op(),
                        kernel_mmio_op(),
                    )
                    .expect("create e1000"),
                );
            }
        }

        None
    }
}
