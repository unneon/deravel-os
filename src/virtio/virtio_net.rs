use crate::net::arp::{ArpHardwareType, ArpOperation, ArpPacket};
use crate::net::{EtherType, EthernetHeader, MacAddress};
use crate::page::{PAGE_SIZE, PageAligned};
use crate::sbi;
use crate::virtio::queue::{QUEUE_SIZE, Queue, VIRTQ_DESC_F_WRITE};
use crate::virtio::registers::{
    LegacyMmioDeviceRegisters, STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK,
};
use core::net::Ipv4Addr;

pub struct VirtioNet {
    regs: LegacyMmioDeviceRegisters,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Header {
    flags: u8,
    gso_type: u8,
    hdr_len: u16,
    gso_size: u16,
    csum_start: u16,
    csum_offset: u16,
}

const VIRTIO_NET_F_CSUM: u32 = 1 << 0;
const VIRTIO_NET_F_MAC: u32 = 1 << 5;

static mut RECEIVE_QUEUE: PageAligned<Queue> = unsafe { core::mem::zeroed() };
static mut TRANSMIT_QUEUE: PageAligned<Queue> = unsafe { core::mem::zeroed() };
static mut RECEIVE_BUFFERS: [[u8; 1514]; QUEUE_SIZE] = [[0; 1514]; QUEUE_SIZE];

impl VirtioNet {
    pub fn new(base_address: usize) -> VirtioNet {
        let regs = LegacyMmioDeviceRegisters::new(base_address);
        initialize_device(&regs);
        VirtioNet { regs }
    }

    pub fn arp_handshake(&mut self) {
        send_arp_request(&self.regs);
        receive_arp_reply(&self.regs);
    }
}

#[repr(C, packed)]
#[derive(Debug)]
struct Packet {
    virtio_net: Header,
    ethernet: EthernetHeader,
    arp: ArpPacket,
}

fn initialize_device(regs: &LegacyMmioDeviceRegisters) {
    assert_eq!(regs.magic_value(), 0x74726976);
    assert_eq!(regs.version(), 1);
    assert_eq!(regs.device_id(), 1);

    regs.set_device_status(0);
    regs.or_device_status(STATUS_ACKNOWLEDGE);
    regs.or_device_status(STATUS_DRIVER);

    regs.set_device_features_sel(0);
    assert_ne!(regs.device_features() & VIRTIO_NET_F_MAC, 0);
    sbi::console_writeln!(
        "net: virtio_blk-net features 0 to 31 are {:#b}",
        regs.device_features()
    );

    regs.set_driver_features_sel(0);
    regs.set_driver_features(VIRTIO_NET_F_MAC | VIRTIO_NET_F_CSUM);

    regs.set_guest_page_size(PAGE_SIZE as u32);

    unsafe { &RECEIVE_QUEUE }.initialize(0, regs);
    unsafe { &TRANSMIT_QUEUE }.initialize(1, regs);

    let receive_queue = unsafe { &mut RECEIVE_QUEUE };
    for (index, descriptor) in receive_queue.descriptors.iter_mut().enumerate() {
        descriptor.address = unsafe { &raw mut RECEIVE_BUFFERS[index] } as u64;
        descriptor.length = 1514;
        descriptor.flags = VIRTQ_DESC_F_WRITE;
    }

    regs.or_device_status(STATUS_DRIVER_OK);
    sbi::console_writeln!(
        "net: device status after initialization {:#b}",
        regs.device_status()
    );
}

fn send_arp_request(regs: &LegacyMmioDeviceRegisters) {
    let mac_address = unsafe { ((regs.base_address + 0x100) as *const MacAddress).read_volatile() };
    sbi::console_writeln!("net: mac address {mac_address}");

    sbi::console_writeln!("net: ARP request has size {}", size_of::<Packet>());
    let mut packet: Packet = unsafe { core::mem::zeroed() };
    packet.ethernet.mac_destination = MacAddress::BROADCAST;
    packet.ethernet.mac_source = mac_address;
    packet.ethernet.ethertype = EtherType::Arp.into();
    packet.arp.htype = ArpHardwareType::Ethernet.into();
    packet.arp.ptype = EtherType::IpV4.into();
    packet.arp.hlen = 6;
    packet.arp.plen = 4;
    packet.arp.oper = ArpOperation::Request.into();
    packet.arp.sender_mac = mac_address;
    packet.arp.sender_ip = Ipv4Addr::new(192, 168, 100, 2);
    packet.arp.target_mac = MacAddress([0; 6]);
    packet.arp.target_ip = Ipv4Addr::new(192, 168, 100, 1);

    let transmit_queue = unsafe { &mut *TRANSMIT_QUEUE };
    transmit_queue.descriptors[0].address = &packet as *const _ as u64;
    transmit_queue.descriptors[0].length = size_of::<Packet>() as u32;

    transmit_queue.send_and_recv(0, 1, regs);
}

fn receive_arp_reply(regs: &LegacyMmioDeviceRegisters) {
    let receive_queue = unsafe { &mut *RECEIVE_QUEUE };
    receive_queue.send_and_recv(0, 0, regs);

    let response_length = receive_queue.used.ring[0].len;
    sbi::console_writeln!("net: ARP response has length {response_length}");

    let response = unsafe { &*((&raw const RECEIVE_BUFFERS[0]) as *const Packet) };
    sbi::console_writeln!("net: received ARP response {response:#?}");
}
