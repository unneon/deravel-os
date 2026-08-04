use crate::util::{ArrayCStr, Padding};

#[derive(Debug)]
#[repr(C, packed)]
pub struct Directory {
    pub name: ArrayCStr<11>,
    pub attr: u8,
    pub nt_res: Padding<1>,
    pub crt_time_tenth: u8,
    pub crt_time: u16,
    pub crt_date: u16,
    pub lst_acc_date: u16,
    pub fst_clus_hi: u16,
    pub wrt_time: u16,
    pub wrt_date: u16,
    pub fst_clus_lo: u16,
    pub file_size: u32,
}

const _: () = assert!(size_of::<Directory>() == 32);
