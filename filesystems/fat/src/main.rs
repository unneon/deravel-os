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

#[derive(Debug, Eq, PartialEq)]
enum Type {
    Fat12,
    Fat16,
    Fat32,
}

#[allow(unused)]
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

    let root_dir_sectors = (common.root_ent_cnt as u32 * 32).div_ceil(common.byts_per_sec as u32);
    let fat_sz = if common.fat_sz_16 != 0 {
        common.fat_sz_16 as u32
    } else {
        bpb.as_extended_32().fat_sz_32
    };

    let tot_sec = if common.tot_sec_16 != 0 {
        common.tot_sec_16 as u32
    } else {
        common.tot_sec_32
    };

    let data_sec =
        tot_sec - (common.rsvd_sec_cnt as u32 + common.num_fats as u32 * fat_sz) + root_dir_sectors;

    let count_of_clusters = data_sec / common.sec_per_clus as u32;

    let type_ = if count_of_clusters < 4085 {
        Type::Fat12
    } else if count_of_clusters < 65525 {
        Type::Fat16
    } else {
        Type::Fat32
    };

    // TODO: Validate the extended BPB.

    let max_valid_cluster_number = count_of_clusters + 1;
    let count_of_clusters_including_two_reserved = count_of_clusters + 2;

    debug!("fat count is {}, fat size is {}", common.num_fats, fat_sz);

    assert_eq!(type_, Type::Fat32);

    let fat_region_start = common.rsvd_sec_cnt as usize;

    let server = Server;
    let mut dispatch = Dispatch::new_object(server, 0);
    dispatch.run();
}

#[allow(dead_code)]
fn concat_path<'a>(prefix: &'a str, suffix: &'a str) -> Cow<'a, str> {
    if prefix.is_empty() {
        suffix.into()
    } else {
        format!("{prefix}/{suffix}").into()
    }
}

app! { main }
