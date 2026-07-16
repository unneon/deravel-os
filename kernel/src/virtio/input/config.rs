use crate::util::volatile::{Volatile, volatile_struct};
use crate::virtio::input::types::{AbsInfo, ConfigSelect, Devids};

volatile_struct! { pub Config
    select: ReadWrite u8,
    subsel: ReadWrite u8,
    size: Readonly u8,
    reserved: Readonly [u8; 5],
    u: Readonly ConfigU,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub union ConfigU {
    pub string: [u8; 128],
    pub bitmap: [u8; 128],
    pub abs: AbsInfo,
    pub ids: Devids,
}

pub fn config_str<'a>(dev: &'a mut Volatile<Config>, select: ConfigSelect, subsel: u8) -> &'a str {
    dev.select().write(select as u8);
    dev.subsel().write(subsel);
    let len = dev.size().read();
    let string = unsafe { &dev.u().assume_pure_reads().string };
    str::from_utf8(&string[..len as usize - 1]).unwrap()
}

pub fn config_absinfo<'a>(dev: &'a mut Volatile<Config>, axis: u16) -> &'a AbsInfo {
    dev.select().write(ConfigSelect::AbsInfo as u8);
    dev.subsel().write(axis as u8);
    unsafe { &dev.u().assume_pure_reads().abs }
}
