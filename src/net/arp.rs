use crate::net::{BigEndian, EtherType, MacAddress, endianness};
use core::net::Ipv4Addr;

#[repr(u16)]
#[derive(Debug, Clone, Copy)]
pub enum ArpHardwareType {
    #[allow(dead_code)]
    Undefined = 0,
    Ethernet = 1,
}
endianness!(ArpHardwareType u16);

#[repr(u16)]
#[derive(Debug, Clone, Copy)]
pub enum ArpOperation {
    #[allow(dead_code)]
    Undefined = 0,
    Request = 1,
    #[allow(dead_code)]
    Reply = 2,
}
endianness!(ArpOperation u16);

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ArpPacket {
    pub htype: BigEndian<ArpHardwareType>,
    pub ptype: BigEndian<EtherType>,
    pub hlen: u8,
    pub plen: u8,
    pub oper: BigEndian<ArpOperation>,
    pub sender_mac: MacAddress,
    pub sender_ip: Ipv4Addr,
    pub target_mac: MacAddress,
    pub target_ip: Ipv4Addr,
}
