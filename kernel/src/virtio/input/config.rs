use crate::util::forward_fmt;
use crate::util::volatile::{Volatile, volatile_struct};
use crate::virtio::input::types::{AbsInfo, ConfigSelect, Devids};

volatile_struct! { pub Config
    select: ReadWrite u8,
    subsel: ReadWrite u8,
    size: Readonly u8,
    reserved: Readonly [u8; 5],
    u: Readonly ConfigU,
}

pub struct ConfigString {
    u: ConfigU,
    len: u8,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub union ConfigU {
    pub string: [u8; 128],
    pub bitmap: [u8; 128],
    pub abs: AbsInfo,
    pub ids: Devids,
}

impl ConfigString {
    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(&self.u.string[..self.len as usize]) }
    }
}

forward_fmt! { impl Debug, Display for ConfigString as as_str; }

pub fn config_str(device: Volatile<Config>, select: ConfigSelect, subsel: u8) -> ConfigString {
    device.select().write(select as u8);
    device.subsel().write(subsel);
    let u = device.u().read();
    let len = device.size().read();
    ConfigString { u, len }
}
