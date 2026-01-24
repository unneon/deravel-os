use crate::page::{PAGE_SIZE, PageAligned};
use crate::sbi;
use crate::virtio::queue::Queue;
use crate::virtio::registers::{
    LegacyMmioDeviceRegisters, STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK,
};
use smoltcp::wire::{
    ArpHardware, ArpOperation, ArpPacket, EthernetAddress, EthernetFrame, EthernetProtocol,
    Ipv4Address,
};

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct Packet<T> {
    header: Header,
    payload: T,
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
    let mac_address: EthernetAddress =
        unsafe { ((regs.base_address + 0x100) as *const EthernetAddress).read_volatile() };
    sbi::console_writeln!("net: mac address {mac_address}");

    let mut packet = Packet {
        header: Header::default(),
        payload: [0; 42],
    };
    let mut eth = EthernetFrame::new_unchecked(&mut packet.payload);
    eth.set_dst_addr(EthernetAddress::BROADCAST);
    eth.set_src_addr(mac_address);
    eth.set_ethertype(EthernetProtocol::Arp);
    let mut arp = ArpPacket::new_unchecked(eth.payload_mut());
    arp.set_hardware_type(ArpHardware::Ethernet);
    arp.set_protocol_type(EthernetProtocol::Ipv4);
    arp.set_hardware_len(6);
    arp.set_protocol_len(4);
    arp.set_operation(ArpOperation::Request);
    arp.set_source_hardware_addr(mac_address.as_bytes());
    arp.set_source_protocol_addr(&[192, 168, 100, 2]);
    arp.set_target_hardware_addr(&[0; 6]);
    arp.set_target_protocol_addr(&[192, 168, 100, 1]);

    let transmit_queue = unsafe { &mut *TRANSMIT_QUEUE };
    transmit_queue.descriptor_readonly(0, &packet, None);
    transmit_queue.send_and_recv(0, 1, regs);
}

fn receive_arp_reply(regs: &LegacyMmioDeviceRegisters) {
    let mut packet: Packet<[u8; 42]> = unsafe { core::mem::zeroed() };

    let receive_queue = unsafe { &mut *RECEIVE_QUEUE };
    receive_queue.descriptor_writeonly(0, &mut packet, None);
    receive_queue.send_and_recv(0, 0, regs);

    sbi::console_writeln!("net: received virtio... {:?}", packet.header);
    let eth = EthernetFrame::new_checked(&packet.payload).unwrap();
    sbi::console_writeln!(
        "net: received ethernet... dst={} src={} ethtype={}",
        eth.dst_addr(),
        eth.src_addr(),
        eth.ethertype()
    );
    let arp = ArpPacket::new_checked(eth.payload()).unwrap();
    sbi::console_writeln!(
        "net: received arp htype={:?} ptype={} oper={:?} sh={} sp={} dh={} dp={}",
        arp.hardware_type(),
        arp.protocol_type(),
        arp.operation(),
        EthernetAddress::from_bytes(arp.source_hardware_addr()),
        Ipv4Address::from_octets(arp.source_protocol_addr().try_into().unwrap()),
        EthernetAddress::from_bytes(arp.target_hardware_addr()),
        Ipv4Address::from_octets(arp.target_protocol_addr().try_into().unwrap()),
    );
}
