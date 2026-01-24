use core::fmt::Formatter;
use core::marker::PhantomData;
use core::mem::{MaybeUninit, transmute_copy};
use core::net::Ipv4Addr;

macro endianness($type:ident $size:ident) {
    unsafe impl Endianness for $type {
        type Size = $size;
        fn to_be(value: $size) -> $size {
            $size::to_be(value)
        }
        fn from_be(value: $size) -> $size {
            $size::from_be(value)
        }
    }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe trait Endianness: Clone + Copy {
    type Size;
    fn to_be(value: Self::Size) -> Self::Size;
    fn from_be(value: Self::Size) -> Self::Size;
}

#[repr(u16)]
#[derive(Debug, Clone, Copy)]
pub enum ArpHardwareType {
    #[allow(dead_code)]
    Undefined = 0,
    Ethernet = 1,
}
endianness!(ArpHardwareType u16);

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ArpPacket {
    pub htype: BigEndian<ArpHardwareType>,
    pub ptype: BigEndian<EtherType>,
    pub hlen: u8,
    pub plen: u8,
    pub oper: BigEndian<u16>,
    pub sender_mac: MacAddress,
    pub sender_ip: Ipv4Addr,
    pub target_mac: MacAddress,
    pub target_ip: Ipv4Addr,
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct BigEndian<T: Copy> {
    bytes: MaybeUninit<T>,
    _phantom: PhantomData<T>,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct EthernetHeader {
    pub mac_destination: MacAddress,
    pub mac_source: MacAddress,
    pub ethertype: BigEndian<EtherType>,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug)]
pub enum EtherType {
    #[allow(dead_code)]
    Undefined = 0,
    IpV4 = 0x0800,
    Arp = 0x0806,
}
endianness!(EtherType u16);

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct MacAddress(pub [u8; 6]);

impl<T: Endianness> BigEndian<T> {
    pub fn get(&self) -> T {
        unsafe {
            transmute_copy::<T::Size, T>(&T::from_be(transmute_copy::<BigEndian<T>, T::Size>(self)))
        }
    }
}

impl MacAddress {
    pub const BROADCAST: MacAddress = MacAddress([0xFF; 6]);
}

endianness!(u16 u16);

impl<T: Endianness> From<T> for BigEndian<T> {
    fn from(value: T) -> Self {
        unsafe {
            transmute_copy::<T::Size, BigEndian<T>>(&T::to_be(transmute_copy::<T, T::Size>(&value)))
        }
    }
}

impl<T: Endianness + core::fmt::Debug> core::fmt::Debug for BigEndian<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        <T as core::fmt::Debug>::fmt(&self.get(), f)
    }
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

impl core::fmt::Debug for MacAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self}")
    }
}
