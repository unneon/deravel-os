use crate::PAGE_SIZE;
use crate::virtio::queue::{QUEUE_SIZE, Queue};
use crate::virtio::registers::{
    Driver, Mmio, Registers, STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK, features, mmio,
};
use core::marker::PhantomData;
use log::{debug, error};
use smoltcp::iface::{Interface, SocketSet, SocketStorage};
use smoltcp::phy::{DeviceCapabilities, Medium};
use smoltcp::socket::dns;
use smoltcp::socket::dns::{DnsQuery, GetQueryResultError};
use smoltcp::time::Instant;
use smoltcp::wire::{
    DnsQueryType, EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address,
};

mmio! { pub Config
    0x000 mac: Readonly EthernetAddress,
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

pub struct PacketReceiveToken<'a>(Mmio<Registers<VirtioNet>>, PhantomData<&'a mut ()>);

pub struct PacketTransmitToken<'a>(Mmio<Registers<VirtioNet>>, PhantomData<&'a ()>);

pub struct VirtioNet {
    regs: Mmio<Registers<VirtioNet>>,
}

static mut RECEIVE_QUEUE: Queue = unsafe { core::mem::zeroed() };
static mut RECEIVE_BUFFERS: [Packet<[u8; 1514]>; QUEUE_SIZE] = unsafe { core::mem::zeroed() };
static mut TRANSMIT_QUEUE: Queue = unsafe { core::mem::zeroed() };
static mut TRANSMIT_BUFFERS: [Packet<[u8; 1514]>; QUEUE_SIZE] = unsafe { core::mem::zeroed() };

impl VirtioNet {
    pub fn new(regs: Mmio<Registers<VirtioNet>>) -> VirtioNet {
        initialize_device(regs);
        VirtioNet { regs }
    }

    pub fn demo(&mut self) {
        let mut iface = Interface::new(
            smoltcp::iface::Config::new(HardwareAddress::Ethernet(self.regs.config().mac().read())),
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

impl Driver for VirtioNet {
    type Config = Config;
}

impl smoltcp::phy::Device for VirtioNet {
    type RxToken<'a> = PacketReceiveToken<'a>;
    type TxToken<'a> = PacketTransmitToken<'a>;

    fn receive(&mut self, _: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        riscv::asm::fence();
        let receive_queue = unsafe { &mut RECEIVE_QUEUE };
        if receive_queue.available.index == receive_queue.used.index + QUEUE_SIZE as u16 {
            return None;
        }
        let transmit_queue = unsafe { &mut TRANSMIT_QUEUE };
        if transmit_queue.available.index == transmit_queue.used.index + QUEUE_SIZE as u16 {
            return None;
        }

        Some((
            PacketReceiveToken(self.regs, PhantomData),
            PacketTransmitToken(self.regs, PhantomData),
        ))
    }

    fn transmit(&mut self, _: Instant) -> Option<Self::TxToken<'_>> {
        riscv::asm::fence();
        let transmit_queue = unsafe { &mut TRANSMIT_QUEUE };
        if transmit_queue.available.index == transmit_queue.used.index + QUEUE_SIZE as u16 {
            return None;
        }

        Some(PacketTransmitToken(self.regs, PhantomData))
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
        let receive_queue = unsafe { &mut RECEIVE_QUEUE };
        let ring_index = receive_queue.available.index as usize % QUEUE_SIZE;
        let used_element = &receive_queue.used.ring[ring_index];
        let descriptor_index = used_element.id as usize;
        let packet = unsafe { &RECEIVE_BUFFERS[descriptor_index] };
        let payload_length = used_element.len as usize - size_of::<Header>();
        let payload = &packet.payload[..payload_length];
        let result = f(payload);

        receive_queue.available.index += 1;
        riscv::asm::fence();
        self.0.queue_notify().write(0);

        result
    }
}

impl smoltcp::phy::TxToken for PacketTransmitToken<'_> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        let transmit_queue = unsafe { &mut TRANSMIT_QUEUE };
        let index = transmit_queue.available.index as usize % QUEUE_SIZE;
        let packet = unsafe { &mut TRANSMIT_BUFFERS[index] };
        let result = f(&mut packet.payload[..len]);

        transmit_queue.descriptors[index].length = (size_of::<Header>() + len) as u32;
        transmit_queue.available.index += 1;
        riscv::asm::fence();
        self.0.queue_notify().write(1);

        result
    }
}

fn initialize_device(regs: Mmio<Registers<VirtioNet>>) {
    regs.status().write(0);
    regs.status().or(STATUS_ACKNOWLEDGE);
    regs.status().or(STATUS_DRIVER);

    let device_features = regs.device_features();
    assert!(device_features.has_mac());

    let mut driver_features = Features::default();
    driver_features.enable_mac();
    regs.driver_features_write(driver_features);

    regs.guest_page_size().write(PAGE_SIZE as u32);

    unsafe { &RECEIVE_QUEUE }.initialize(0, regs);
    unsafe { &TRANSMIT_QUEUE }.initialize(1, regs);

    initialize_receive_buffers(regs);
    initialize_transmit_buffers();

    regs.status().or(STATUS_DRIVER_OK);
}

fn initialize_receive_buffers(regs: Mmio<Registers<VirtioNet>>) {
    let queue = unsafe { &mut RECEIVE_QUEUE };
    for (i, buffer) in unsafe { RECEIVE_BUFFERS.iter_mut() }.enumerate() {
        queue.available.ring[i] = i as u16;
        queue.descriptor_writeonly(i as u16, buffer, None);
    }
    riscv::asm::fence();
    queue.available.index = QUEUE_SIZE as u16;
    riscv::asm::fence();
    regs.queue_notify().write(0);
}

fn initialize_transmit_buffers() {
    let queue = unsafe { &mut TRANSMIT_QUEUE };
    for (i, buffer) in unsafe { TRANSMIT_BUFFERS.iter() }.enumerate() {
        queue.available.ring[i] = i as u16;
        queue.descriptor_readonly(i as u16, buffer, None);
    }
    riscv::asm::fence();
}
