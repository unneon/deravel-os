#[derive(Clone, Copy)]
#[repr(C)]
pub struct AbsInfo {
    pub min: u32,
    pub max: u32,
    pub fuzz: u32,
    pub flat: u32,
    pub res: u32,
}

#[allow(dead_code)]
#[repr(u8)]
pub enum ConfigSelect {
    Unset = 0x00,
    IdName = 0x01,
    IdSerial = 0x02,
    IdDevids = 0x03,
    PropBits = 0x10,
    EvBits = 0x11,
    AbsInfo = 0x12,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Devids {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}
