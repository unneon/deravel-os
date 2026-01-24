pub mod arp;

use core::marker::PhantomData;
use core::mem::{MaybeUninit, transmute};

macro endianness($type:ident $size:ident) {
    impl From<$type> for BigEndian<$type> {
        fn from(value: $type) -> BigEndian<$type> {
            unsafe {
                transmute::<$size, BigEndian<$type>>($size::to_be(transmute::<$type, $size>(value)))
            }
        }
    }

    impl From<BigEndian<$type>> for $type {
        fn from(value: BigEndian<$type>) -> $type {
            unsafe {
                transmute::<$size, $type>($size::from_be(transmute::<BigEndian<$type>, $size>(
                    value,
                )))
            }
        }
    }
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

impl MacAddress {
    pub const BROADCAST: MacAddress = MacAddress([0xFF; 6]);
}

impl<T: From<BigEndian<T>> + Copy + core::fmt::Debug> core::fmt::Debug for BigEndian<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        <T as core::fmt::Debug>::fmt(&T::from(*self), f)
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
