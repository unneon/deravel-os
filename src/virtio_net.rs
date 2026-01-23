mod registers;
mod structures;

use crate::virtio_net::registers::{
    LegacyMmioDeviceRegisters, STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK,
    VIRTIO_NET_F_CSUM, VIRTIO_NET_F_MAC,
};
use crate::virtio_net::structures::{
    Queue, VIRTIO_BLK_T_IN, VIRTIO_BLK_T_OUT, VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE,
};
use crate::{PAGE_SIZE, PageAligned, sbi};
use core::mem::transmute;

#[repr(C, packed)]
pub struct MacAddress([u8; 6]);

pub struct VirtioNet {
    regs: LegacyMmioDeviceRegisters,
}

const QUEUE_SIZE: usize = 16;
const BROADCAST: MacAddress = MacAddress([0xFF; 6]);

static mut RECEIVE_QUEUE: PageAligned<Queue> = unsafe { core::mem::zeroed() };
static mut TRANSMIT_QUEUE: PageAligned<Queue> = unsafe { core::mem::zeroed() };
static mut RECEIVE_BUFFERS: [[u8; 1514]; QUEUE_SIZE] = [[0; 1514]; QUEUE_SIZE];

impl VirtioNet {
    pub fn new(base_address: usize) -> VirtioNet {
        let regs = LegacyMmioDeviceRegisters::new(base_address);
        initialize_device(&regs);
        VirtioNet { regs }
    }

    // pub fn read(&mut self, sector: u64, buf: &mut [u8; 512]) -> Result<(), VirtioNetError> {
    //     request(sector, buf.as_ptr(), RequestType::Read, &self.regs)
    // }
    //
    // #[allow(dead_code)]
    // pub fn write(&mut self, sector: u64, buf: &[u8; 512]) -> Result<(), VirtioNetError> {
    //     request(sector, buf.as_ptr(), RequestType::Write, &self.regs)
    // }
}

impl core::fmt::Display for MacAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

#[repr(C, packed)]
struct VirtioNetHeader {
    flags: u8,
    gso_type: u8,
    hdr_len: u16,
    gso_size: u16,
    csum_start: u16,
    csum_offset: u16,
}
const VIRTIO_NET_HDR_F_NEEDS_CSUM: u8 = 1;

#[repr(C, packed)]
struct EthernetHeader {
    mac_destination: MacAddress,
    mac_source: MacAddress,
    ethertype: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IpHeader {
    version_and_ihl: u8,
    dscp_and_ecn: u8,
    total_length: u16,
    identification: u16,
    flags_and_fragment_offset: u16,
    time_to_live: u8,
    protocol: u8,
    header_checksum: u16,
    source_address: [u8; 4],
    destination_address: [u8; 4],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct UdpHeader {
    source_port: u16,
    destination_port: u16,
    length: u16,
    checksum: u16,
}

#[repr(C, packed)]
struct DnsHeader {
    id: u16,
    qr_opcode_aa_tc_rd: u8,
    ra_z_rcode: u8,
    qdcount: u16,
    ancount: u16,
    nscount: u16,
    ar_count: u16,
}

#[repr(C, packed)]
struct DnsQuery {
    url: [u8; 12],
    qtype: u16,
    qclass: u16,
}

#[repr(C, packed)]
struct Packet {
    virtio_net: VirtioNetHeader,
    ethernet: EthernetHeader,
    ip: IpHeader,
    udp: UdpHeader,
    dns: DnsHeader,
    dns_query: DnsQuery,
}

impl IpHeader {
    fn compute_checksum(&mut self) {
        // TODO: I don't think this is correct
        self.header_checksum = 0;
        let mut sum = 0;
        for word in unsafe { transmute::<Self, [u16; 10]>(*self) } {
            sum += word as u32;
        }
        self.header_checksum = !(sum as u16 + (sum >> 16) as u16);
    }
}

impl UdpHeader {
    fn compute_partial_checksum(&mut self) {
        // TODO: I don't think this is correct
        self.checksum = 0;
        let mut sum = 0;
        for word in unsafe { transmute::<Self, [u16; 4]>(*self) } {
            sum += word as u32;
        }
        self.checksum = !(sum as u16 + (sum >> 16) as u16);
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
    sbi::console_writeln!(
        "net: virtio-net features 0 to 31 are {:#b}",
        regs.device_features()
    );

    regs.set_driver_features_sel(0);
    regs.set_driver_features(VIRTIO_NET_F_MAC | VIRTIO_NET_F_CSUM);

    regs.set_guest_page_size(PAGE_SIZE as u32);

    initialize_queue(0, &raw const RECEIVE_QUEUE, regs);
    initialize_queue(1, &raw const TRANSMIT_QUEUE, regs);

    #[allow(static_mut_refs)]
    let receive_queue = unsafe { &mut RECEIVE_QUEUE.0 };
    for (index, descriptor) in receive_queue.descriptors.iter_mut().enumerate() {
        descriptor.address = unsafe { &raw mut RECEIVE_BUFFERS[index] } as u64;
        descriptor.length = 1514;
        descriptor.flags = VIRTQ_DESC_F_WRITE;
    }

    let mac_address = unsafe { ((regs.base_address + 0x100) as *const MacAddress).read_volatile() };
    sbi::console_writeln!("net: mac address {mac_address}");

    regs.or_device_status(STATUS_DRIVER_OK);
    sbi::console_writeln!(
        "net: device status after initialization {:#b}",
        regs.device_status()
    );

    sbi::console_writeln!(
        "net: DEBUG virtio-net header size: {}",
        size_of::<VirtioNetHeader>()
    );
    sbi::console_writeln!(
        "net: DEBUG   ethernet header size: {}",
        size_of::<EthernetHeader>()
    );
    sbi::console_writeln!(
        "net: DEBUG         ip header size: {}",
        size_of::<IpHeader>()
    );
    sbi::console_writeln!(
        "net: DEBUG        udp header size: {}",
        size_of::<UdpHeader>()
    );
    sbi::console_writeln!(
        "net: DEBUG               dns size: {}",
        size_of::<DnsHeader>()
    );
    sbi::console_writeln!(
        "net: DEBUG         dns query size: {}",
        size_of::<DnsQuery>()
    );
    sbi::console_writeln!("net: DEBUG            packet size: {}", size_of::<Packet>());
    let mut packet: Packet = unsafe { core::mem::zeroed() };
    packet.virtio_net.flags = VIRTIO_NET_HDR_F_NEEDS_CSUM;
    packet.virtio_net.csum_start = 34;
    packet.virtio_net.csum_offset = 6;
    packet.ethernet.mac_destination = BROADCAST;
    packet.ethernet.mac_source = mac_address;
    packet.ethernet.ethertype = u16::to_be(0x0800);
    packet.ip.version_and_ihl = (4 << 4) | 5;
    packet.ip.total_length = u16::to_be(20 + 8 + 12 + 16);
    packet.ip.time_to_live = 64;
    packet.ip.protocol = 17;
    packet.ip.source_address = [10, 0, 2, 15];
    packet.ip.destination_address = [8, 8, 8, 8];
    packet.ip.compute_checksum();
    packet.udp.source_port = u16::to_be(1024);
    packet.udp.destination_port = u16::to_be(53);
    packet.udp.length = u16::to_be(8 + 12 + 16);
    packet.udp.compute_partial_checksum();
    packet.dns.qdcount = u16::to_be(1);
    packet.dns_query.url = [
        6, b'g', b'o', b'o', b'g', b'l', b'e', 3, b'c', b'o', b'm', b'\0',
    ];
    packet.dns_query.qtype = u16::to_be(1);
    packet.dns_query.qclass = u16::to_be(1);

    #[allow(static_mut_refs)]
    let transmit_queue = unsafe { &mut TRANSMIT_QUEUE.0 };
    transmit_queue.descriptors[0].address = &packet as *const _ as u64;
    transmit_queue.descriptors[0].length = size_of::<Packet>() as u32;

    transmit_queue.available.ring[transmit_queue.available.index as usize % QUEUE_SIZE] = 0;
    transmit_queue.available.index += 1;
    riscv::asm::fence();
    regs.set_queue_notify(1);

    while is_transmit_busy(1) {}

    loop {
        sbi::console_writeln!(
            "tx used.index={}, rx used.index={}",
            unsafe { (&raw const TRANSMIT_QUEUE.0.used.index).read_volatile() },
            unsafe { (&raw const RECEIVE_QUEUE.0.used.index).read_volatile() }
        );
        for _ in 0..1_000_000_000 {
            riscv::asm::nop();
        }
    }
}

fn initialize_queue(
    index: u32,
    queue: *const PageAligned<Queue>,
    regs: &LegacyMmioDeviceRegisters,
) {
    regs.set_queue_sel(index);
    assert_eq!(regs.queue_pfn(), 0);
    assert!(QUEUE_SIZE <= regs.queue_size_max() as usize);
    regs.set_queue_size(QUEUE_SIZE as u32);
    regs.set_queue_align(PAGE_SIZE as u32);
    regs.set_queue_pfn((queue as usize / PAGE_SIZE) as u32);
}

// fn request(
//     sector: u64,
//     buf: *const u8,
//     request_type: RequestType,
//     regs: &LegacyMmioDeviceRegisters,
// ) -> Result<(), VirtioNetError> {
//     let request = RequestHeader {
//         type_: match request_type {
//             RequestType::Write => VIRTIO_BLK_T_OUT,
//             RequestType::Read => VIRTIO_BLK_T_IN,
//         },
//         reserved: 0,
//         sector,
//     };
//     let status: u8 = 0;
//
//     #[allow(static_mut_refs)]
//     let queue = unsafe { &mut VIRTQ.0 };
//     let prev_used_index = queue.used.index;
//
//     queue.descriptors[0].address = &request as *const _ as u64;
//     queue.descriptors[0].length = 16;
//     queue.descriptors[0].flags = VIRTQ_DESC_F_NEXT;
//     queue.descriptors[0].next = 1;
//
//     queue.descriptors[1].address = buf as u64;
//     queue.descriptors[1].length = 512;
//     queue.descriptors[1].flags = VIRTQ_DESC_F_NEXT
//         | match request_type {
//             RequestType::Read => VIRTQ_DESC_F_WRITE,
//             RequestType::Write => 0,
//         };
//     queue.descriptors[1].next = 2;
//
//     queue.descriptors[2].address = &status as *const _ as u64;
//     queue.descriptors[2].length = 1;
//     queue.descriptors[2].flags = VIRTQ_DESC_F_WRITE;
//
//     queue.available.ring[queue.available.index as usize % QUEUE_SIZE] = 0; // first descriptor index
//     queue.available.index += 1;
//     riscv::asm::fence();
//     regs.set_queue_notify(0);
//
//     while is_busy(prev_used_index + 1) {}
//
//     match status {
//         0 => Ok(()),
//         1 => Err(VirtioNetError),
//         _ => unreachable!(),
//     }
// }

fn is_transmit_busy(waiting_for: u16) -> bool {
    (unsafe { (&raw const TRANSMIT_QUEUE.0.used.index).read_volatile() }) != waiting_for
}
