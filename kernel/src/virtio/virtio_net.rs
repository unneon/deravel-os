use crate::util::volatile::{Volatile, volatile_struct};
use crate::virtio::queue::{QUEUE_SIZE, Queue};
use crate::virtio::registers::{STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK, features};
use crate::virtio::{NotifySlot, VirtioCommonConfig};
use log::{debug, error};
use smoltcp::iface::{Interface, SocketSet, SocketStorage};
use smoltcp::phy::{DeviceCapabilities, Medium};
use smoltcp::socket::dns;
use smoltcp::socket::dns::{DnsQuery, GetQueryResultError};
use smoltcp::time::Instant;
use smoltcp::wire::{
    DnsQueryType, EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address,
};

volatile_struct! { pub Config
    mac: Readonly EthernetAddress,
}

features! { VirtioNet Features 0
    has_mac enable_mac 5
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
struct Header {
    flags: u8,
    gso_type: u8,
    hdr_len: u16,
    gso_size: u16,
    csum_start: u16,
    csum_offset: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct Packet<T> {
    header: Header,
    payload: T,
}

pub struct PacketReceiveToken<'a>(&'a mut Queue);

pub struct PacketTransmitToken<'a>(&'a mut Queue);

pub struct VirtioNet {
    device: Volatile<Config>,
    rx_queue: Queue,
    tx_queue: Queue,
}

static mut RECEIVE_BUFFERS: [Packet<[u8; 1514]>; QUEUE_SIZE] = unsafe { core::mem::zeroed() };
static mut TRANSMIT_BUFFERS: [Packet<[u8; 1514]>; QUEUE_SIZE] = unsafe { core::mem::zeroed() };

impl VirtioNet {
    pub fn new(
        common: Volatile<VirtioCommonConfig>,
        notify: NotifySlot,
        device: Volatile<Config>,
    ) -> VirtioNet {
        let (rx_queue, tx_queue) = initialize_device(common, notify);
        VirtioNet {
            device,
            rx_queue,
            tx_queue,
        }
    }

    pub fn demo(&mut self) {
        let mut iface = Interface::new(
            smoltcp::iface::Config::new(HardwareAddress::Ethernet(self.device.mac().read())),
            self,
            Instant::from_secs(0),
        );
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs
                .push(IpCidr::new(IpAddress::v4(192, 168, 100, 2), 24))
                .unwrap();
        });
        iface
            .routes_mut()
            .add_default_ipv4_route(Ipv4Address::new(192, 168, 100, 1))
            .unwrap();
        let servers = [Ipv4Address::new(8, 8, 8, 8).into()];
        let mut queries_buf: [Option<DnsQuery>; 1] = Default::default();
        let socket = dns::Socket::new(&servers, queries_buf.as_mut_slice());
        let mut sockets_storage: [SocketStorage; 1] = Default::default();
        let mut sockets = SocketSet::new(sockets_storage.as_mut_slice());
        let dns_handle = sockets.add(socket);
        let socket = sockets.get_mut::<dns::Socket>(dns_handle);
        let domain = "cegla.net";
        let query = socket
            .start_query(iface.context(), domain, DnsQueryType::A)
            .unwrap();
        loop {
            let timestamp = Instant::from_secs(0);
            iface.poll(timestamp, self, &mut sockets);

            match sockets
                .get_mut::<dns::Socket>(dns_handle)
                .get_query_result(query)
            {
                Ok(addrs) => {
                    debug!("dns query of {domain} resolved with {addrs:?}");
                    break;
                }
                Err(GetQueryResultError::Pending) => {}
                Err(e) => {
                    error!("dns query failed: {e:?}");
                    break;
                }
            }
        }
    }
}

impl smoltcp::phy::Device for VirtioNet {
    type RxToken<'a> = PacketReceiveToken<'a>;
    type TxToken<'a> = PacketTransmitToken<'a>;

    fn receive(&mut self, _: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        riscv::asm::fence();
        if self.rx_queue.available.index == self.rx_queue.used.index + QUEUE_SIZE as u16 {
            return None;
        }
        if self.tx_queue.available.index == self.tx_queue.used.index + QUEUE_SIZE as u16 {
            return None;
        }

        Some((
            PacketReceiveToken(&mut self.rx_queue),
            PacketTransmitToken(&mut self.tx_queue),
        ))
    }

    fn transmit(&mut self, _: Instant) -> Option<Self::TxToken<'_>> {
        riscv::asm::fence();
        if self.tx_queue.available.index == self.tx_queue.used.index + QUEUE_SIZE as u16 {
            return None;
        }

        Some(PacketTransmitToken(&mut self.tx_queue))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = 1500;
        caps
    }
}

impl smoltcp::phy::RxToken for PacketReceiveToken<'_> {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R {
        let ring_index = self.0.available.index as usize % QUEUE_SIZE;
        let used_element = &self.0.used.ring[ring_index];
        let descriptor_index = used_element.id as usize;
        let packet = unsafe { &RECEIVE_BUFFERS[descriptor_index] };
        let payload_length = used_element.len as usize - size_of::<Header>();
        let payload = &packet.payload[..payload_length];
        let result = f(payload);

        self.0.available.index += 1;
        riscv::asm::fence();
        self.0.notify.write(0);

        result
    }
}

impl smoltcp::phy::TxToken for PacketTransmitToken<'_> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        let index = self.0.available.index as usize % QUEUE_SIZE;
        let packet = unsafe { &mut TRANSMIT_BUFFERS[index] };
        let result = f(&mut packet.payload[..len]);

        self.0.descriptors[index].length = (size_of::<Header>() + len) as u32;
        self.0.available.index += 1;
        riscv::asm::fence();
        self.0.notify.write(1);

        result
    }
}

fn initialize_device(common: Volatile<VirtioCommonConfig>, notify: NotifySlot) -> (Queue, Queue) {
    common.device_status().write(0);
    common.device_status().write_bitor(STATUS_ACKNOWLEDGE as u8);
    common.device_status().write_bitor(STATUS_DRIVER as u8);

    common.device_feature_select().write(0);
    let host_features = Features(common.device_feature().read());
    assert!(host_features.has_mac());

    let mut driver_features = Features::default();
    driver_features.enable_mac();
    common.driver_feature_select().write(0);
    common.driver_feature().write(driver_features.into());

    let mut rx_queue = Queue::new(0, common, &notify, QUEUE_SIZE);
    let mut tx_queue = Queue::new(1, common, &notify, QUEUE_SIZE);

    initialize_receive_buffers(&mut rx_queue);
    initialize_transmit_buffers(&mut tx_queue);

    common.device_status().write_bitor(STATUS_DRIVER_OK as u8);

    (rx_queue, tx_queue)
}

fn initialize_receive_buffers(rx_queue: &mut Queue) {
    for (i, buffer) in unsafe { RECEIVE_BUFFERS.iter_mut() }.enumerate() {
        rx_queue.available.ring[i] = i as u16;
        rx_queue.descriptor_writeonly(i as u16, buffer, None);
    }
    riscv::asm::fence();
    rx_queue.available.index = QUEUE_SIZE as u16;
    riscv::asm::fence();
    rx_queue.notify.write(rx_queue.index);
}

fn initialize_transmit_buffers(tx_queue: &mut Queue) {
    for (i, buffer) in unsafe { TRANSMIT_BUFFERS.iter() }.enumerate() {
        tx_queue.available.ring[i] = i as u16;
        tx_queue.descriptor_readonly(i as u16, buffer, None);
    }
    riscv::asm::fence();
}
