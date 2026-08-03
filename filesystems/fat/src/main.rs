#![no_std]
#![no_main]

extern crate alloc;

mod bpb;
mod util;

use crate::bpb::{Bpb, SECTOR_SIZE};
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use core::assert_matches;
use deravel_kernel_api::*;
use log::*;

struct Server;

impl FilesystemServer<usize> for Server {
    fn read(&mut self, _: &mut Ctx<Self>, cap: usize, path_suffix: &str) -> Vec<u8> {
        todo!()
    }

    fn read_large(
        &mut self,
        ctx: &mut Ctx<Self>,
        cap: usize,
        path_suffix: &str,
    ) -> Capability<SharedMemory> {
        todo!()
    }

    fn write(&mut self, _: &mut Ctx<Self>, cap: usize, path_suffix: &str, data: &[u8]) {
        todo!()
    }

    fn subcapability(
        &mut self,
        ctx: &mut Ctx<Self>,
        cap: usize,
        path_suffix: &str,
    ) -> Capability<Filesystem> {
        todo!()
    }
}

fn main(args: TarFsArgs) {
    let bpb = Bpb {
        bytes: *Box::try_from(args.drive.read(0)).unwrap(),
    };

    let common = bpb.as_common();

    assert_matches!(common.bs_jmp_boot, [0xEB, _, 0x90] | [0xE9, _, _]);
    // common.bs_oem_name has no invariant.

    assert_matches!({ common.byts_per_sec }, 512 | 1024 | 2048 | 4096);
    if common.byts_per_sec as usize != SECTOR_SIZE {
        unimplemented!("unsupported sector size")
    }

    assert!(common.sec_per_clus.is_power_of_two() && common.sec_per_clus > 0);
    assert_ne!({ common.rsvd_sec_cnt }, 0);
    // common.num_fats has no invariant.
    // common.root_ent_cnt and common.tot_sec_16 are validated once FAT type is determined.
    assert_matches!(common.media, 0xF0 | 0xF8..=0xFF);
    // common.fats_z16 is validated once FAT type is determined.
    // common.sec_per_trk has no invariant.
    // common.num_heads has no invariant.
    // TODO: Validate common.hidd_sec.
    // common.tot_sec_32 validated once FAT type is determined.

    // Calculation written exactly as in spec. (Microsoft FAT Specification 3.5).

    // TODO: Can this use u32? Does that have good instructions on RISC-V 64?
    #[allow(clippy::manual_div_ceil)]
    let root_dir_sectors = ((common.root_ent_cnt as usize * 32)
        + (common.byts_per_sec as usize - 1))
        / common.byts_per_sec as usize;
    let fats_z = if common.fats_z16 != 0 {
        common.fats_z16 as usize
    } else {
        bpb.as_extended32().fats_z32 as usize
    };

    let tot_sec = if common.tot_sec_16 != 0 {
        common.tot_sec_16 as usize
    } else {
        common.tot_sec_32 as usize
    };

    let data_sec = tot_sec - (common.rsvd_sec_cnt as usize + common.num_fats as usize * fats_z)
        + root_dir_sectors;

    let count_of_clusters = data_sec / common.sec_per_clus as usize;

    if count_of_clusters < 4085 {
        unimplemented!("FAT12")
    } else if count_of_clusters < 65525 {
        unimplemented!("FAT16")
    } else {
        let extended32 = bpb.as_extended32();
        debug!("{extended32:?}");
        // TODO: Validate the extended BPB.
    }

    let server = Server;
    let mut dispatch = Dispatch::new_object(server, 0);
    dispatch.run();
}

fn concat_path<'a>(prefix: &'a str, suffix: &'a str) -> Cow<'a, str> {
    if prefix.is_empty() {
        suffix.into()
    } else {
        format!("{prefix}/{suffix}").into()
    }
}

app! { main }
