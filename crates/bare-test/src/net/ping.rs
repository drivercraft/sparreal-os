extern crate alloc;

use alloc::vec;
use core::time::Duration;

use rd_net::{Interface, Net, NetError, RxPacket, RxQueue, TxQueue};
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
    let ms = crate::os::time::since_boot().as_millis() as i64;
    Instant::from_millis(ms)
}

fn spin_delay(duration: Duration) {
    let start = crate::os::time::since_boot();
    while crate::os::time::since_boot().saturating_sub(start) < duration {
        core::hint::spin_loop();
    }
}

struct BridgeDevice {
    tx: TxQueue,
    rx: RxQueue,
}

struct NetRxToken<'a> {
    packet: RxPacket<'a>,
}

impl RxToken for NetRxToken<'_> {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        self.packet.consume(f)
    }
}

struct NetTxToken<'a> {
    tx: &'a mut TxQueue,
}

impl TxToken for NetTxToken<'_> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let (ret, mut pending) = match self.tx.prepare_send(len, f) {
            Ok(result) => result,
            Err(err) => panic!("tx prepare failed: {err:?}"),
        };

        loop {
            match pending.try_submit() {
                Ok(()) => return ret,
                Err(NetError::Retry) => spin_delay(Duration::from_millis(1)),
                Err(err) => panic!("tx submit failed: {err:?}"),
            }
        }
    }
}

impl Device for BridgeDevice {
    type RxToken<'a>
        = NetRxToken<'a>
    where
        Self: 'a;
    type TxToken<'a>
        = NetTxToken<'a>
    where
        Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let BridgeDevice { tx, rx } = self;
        let packet = rx.try_receive()?;
        Some((NetRxToken { packet }, NetTxToken { tx }))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(NetTxToken { tx: &mut self.tx })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = self.tx.buf_size();
        caps.medium = Medium::Ethernet;
        caps.max_burst_size = Some(1);
        caps
    }
}

pub fn run_ping_test(nic: impl Interface) {
    let mut net = Net::new(nic, crate::os::mem::dma::kernel_dma_op());
    let mac = net.mac_address();
    let tx = net.create_tx_queue().expect("create tx queue");
    let rx = net.create_rx_queue().expect("create rx queue");
    let mut dev = BridgeDevice { tx, rx };

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
            crate::println!("ping_test: icmp echo request sent");
        }

        if sent
            && socket.can_recv()
            && let Ok((_data, addr)) = socket.recv()
        {
            crate::println!("ping_test: icmp echo reply from {addr:?}");
            received = true;
            break;
        }

        spin_delay(Duration::from_millis(10));
    }

    assert!(received, "ping_test: no icmp echo reply received");
}
