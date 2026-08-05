use crate::util::{ArrayCStr, Padding};
use core::ops::Deref;

#[repr(C, align(512))]
pub union Bpb {
    pub common: BpbCommon,
    pub extended_12_16: BpbExtended1216,
    pub extended_32: BpbExtended32,
    pub bytes: [u8; DISK_SECTOR_SIZE],
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct BpbCommon {
    pub bs_jmp_boot: [u8; 3],
    pub bs_oem_name: ArrayCStr<8>,
    pub byts_per_sec: u16,
    pub sec_per_clus: u8,
    pub rsvd_sec_cnt: u16,
    pub num_fats: u8,
    pub root_ent_cnt: u16,
    pub tot_sec_16: u16,
    pub media: u8,
    pub fat_sz_16: u16,
    pub sec_per_trk: u16,
    pub num_heads: u16,
    pub hidd_sec: u32,
    pub tot_sec_32: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct BpbExtended1216 {
    common: BpbCommon,
    pub bs_drv_num: u8,
    bs_reserved1: Padding<1>,
    pub bs_boot_sig: u8,
    pub bs_vol_id: u32,
    pub bs_vol_lab: ArrayCStr<11>,
    pub bs_fil_sys_type: ArrayCStr<8>,
    _0: Padding<448>,
    pub signature_word: u16,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct BpbExtended32 {
    common: BpbCommon,
    pub fat_sz_32: u32,
    pub ext_flags: u16,
    pub fs_ver: u16,
    pub root_clus: u32,
    pub fs_info: u16,
    pub bk_boot_sec: u16,
    reserved: Padding<12>,
    pub bs_drv_num: u8,
    bs_reserved1: Padding<1>,
    pub bs_boot_sig: u8,
    pub bs_vol_id: u32,
    pub bs_vol_lab: ArrayCStr<11>,
    pub bs_fil_sys_type: ArrayCStr<8>,
    _0: Padding<420>,
    pub bs_signature_word: u16,
}

const _: () = assert!(size_of::<BpbCommon>() == 36);
const _: () = assert!(size_of::<BpbExtended1216>() == 512);
const _: () = assert!(size_of::<BpbExtended32>() == 512);

pub const DISK_SECTOR_SIZE: usize = 512;

impl Bpb {
    pub fn as_common(&self) -> &BpbCommon {
        // SAFETY: BPB is plain old data.
        unsafe { &self.common }
    }

    pub fn as_extended_12_16(&self) -> &BpbExtended1216 {
        // SAFETY: BPB is plain old data.
        unsafe { &self.extended_12_16 }
    }

    pub fn as_extended_32(&self) -> &BpbExtended32 {
        // SAFETY: BPB is plain old data.
        unsafe { &self.extended_32 }
    }
}

impl Deref for Bpb {
    type Target = BpbCommon;

    fn deref(&self) -> &BpbCommon {
        self.as_common()
    }
}
