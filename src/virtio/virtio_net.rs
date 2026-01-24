use crate::net::arp::{ArpHardwareType, ArpOperation, ArpPacket};
use crate::net::{EtherType, EthernetHeader, MacAddress};
use crate::page::{PAGE_SIZE, PageAligned};
use crate::sbi;
use crate::virtio::queue::Queue;
use crate::virtio::registers::{
    LegacyMmioDeviceRegisters, STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK,
};
use core::net::Ipv4Addr;

#[repr(C, packed)]
#[derive(Debug)]
struct FullArpPacket {
    virtio_net: Header,
    ethernet: EthernetHeader,
    arp: ArpPacket,
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

pub struct VirtioNet {
    regs: LegacyMmioDeviceRegisters,
}

const VIRTIO_NET_F_MAC: u32 = 1 << 5;

static mut RECEIVE_QUEUE: PageAligned<Queue> = unsafe { core::mem::zeroed() };
static mut TRANSMIT_QUEUE: PageAligned<Queue> = unsafe { core::mem::zeroed() };

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

fn initialize_device(regs: &LegacyMmioDeviceRegisters) {
    assert_eq!(regs.magic_value(), 0x74726976);
    assert_eq!(regs.version(), 1);
    assert_eq!(regs.device_id(), 1);

    regs.set_device_status(0);
    regs.or_device_status(STATUS_ACKNOWLEDGE);
    regs.or_device_status(STATUS_DRIVER);

    regs.set_device_features_sel(0);
    assert_ne!(regs.device_features() & VIRTIO_NET_F_MAC, 0);

    regs.set_driver_features_sel(0);
    regs.set_driver_features(VIRTIO_NET_F_MAC);

    regs.set_guest_page_size(PAGE_SIZE as u32);

    unsafe { &RECEIVE_QUEUE }.initialize(0, regs);
    unsafe { &TRANSMIT_QUEUE }.initialize(1, regs);

    regs.or_device_status(STATUS_DRIVER_OK);
}

fn send_arp_request(regs: &LegacyMmioDeviceRegisters) {
    let mac_address = unsafe { ((regs.base_address + 0x100) as *const MacAddress).read_volatile() };
    sbi::console_writeln!("net: mac address {mac_address}");

    sbi::console_writeln!("net: ARP request has size {}", size_of::<FullArpPacket>());
    let mut packet: FullArpPacket = unsafe { core::mem::zeroed() };
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
    transmit_queue.descriptor_readonly(0, &packet, None);
    transmit_queue.send_and_recv(0, 1, regs);
}

fn receive_arp_reply(regs: &LegacyMmioDeviceRegisters) {
    let mut packet: FullArpPacket = unsafe { core::mem::zeroed() };

    let receive_queue = unsafe { &mut *RECEIVE_QUEUE };
    receive_queue.descriptor_writeonly(0, &mut packet, None);
    receive_queue.send_and_recv(0, 0, regs);

    sbi::console_writeln!("net: received ARP response {packet:#?}");
}
