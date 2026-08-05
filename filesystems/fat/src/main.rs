#![no_std]
#![no_main]

extern crate alloc;

mod bpb;
mod directory_entry;
mod util;

use crate::Type::*;
use crate::bpb::{Bpb, DISK_SECTOR_SIZE};
use crate::directory_entry::DirectoryEntry;
use crate::util::ArrayCStr;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::assert_matches;
use deravel_kernel_api::*;
use log::info;

#[derive(Clone, Copy)]
enum Directory {
    Normal { cluster: u32 },
    RootDirectoryRegion,
}

struct Fat {
    drive: Capability<Drive>,
    bpb: Bpb,
    fat_sz: u32,
    type_: Type,
    max_cluster: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Type {
    Fat12,
    Fat16,
    Fat32,
}

impl Fat {
    fn traverse_path(&self, mut dir: Directory, mut path: &str) -> (u32, usize) {
        'segments: loop {
            let (path_seg, path_tail) = match path.split_once('/') {
                Some((path_seg, path_tail)) => (path_seg, Some(path_tail)),
                None => (path, None),
            };
            let needle = str_to_short_file_name(path_seg);
            for de in self.read_directory(dir) {
                if de.name == needle {
                    let de_cluster = (de.fst_clus_hi as u32) << 16 | de.fst_clus_lo as u32;
                    let Some(path_tail) = path_tail else {
                        return (de_cluster, de.file_size as usize);
                    };
                    dir = Directory::Normal {
                        cluster: de_cluster,
                    };
                    path = path_tail;
                    continue 'segments;
                }
            }
            panic!("file not found")
        }
    }

    fn read_directory(&self, dir: Directory) -> Vec<DirectoryEntry> {
        match dir {
            Directory::Normal { cluster } => self.read_normal_directory(cluster),
            Directory::RootDirectoryRegion => self.read_root_directory_region(),
        }
    }

    fn read_normal_directory(&self, cluster: u32) -> Vec<DirectoryEntry> {
        directory_bytes_to_entries(self.read_file(cluster))
    }

    fn read_root_directory_region(&self) -> Vec<DirectoryEntry> {
        let rdr_sectors_start =
            self.bpb.rsvd_sec_cnt as u64 + self.bpb.num_fats as u64 * self.bpb.fat_sz_16 as u64;
        let rdr_sectors_count = self.bpb.root_ent_cnt as u64 * 32 / DISK_SECTOR_SIZE as u64;
        let mut entries = Vec::new();
        for i in 0..rdr_sectors_count {
            let bytes = self.drive.read(rdr_sectors_start + i);
            entries.extend_from_slice(&directory_bytes_to_entries(bytes));
        }
        entries
    }

    fn read_file(&self, mut cluster: u32) -> Vec<u8> {
        let mut data = Vec::new();
        loop {
            data.extend_from_slice(&self.read_data(cluster));
            let fat_entry = self.read_fat_entry(cluster);
            match (self.type_, fat_entry & ((!0) >> 4)) {
                (Fat16, next_cluster @ 0x0002..) if next_cluster <= self.max_cluster => {
                    cluster = next_cluster;
                }
                (Fat16, 0xFFF8..=0xFFFF) => break data,
                (Fat32, next_cluster @ 0x000_0002..) if next_cluster <= self.max_cluster => {
                    cluster = next_cluster;
                }
                (Fat32, 0xFFF_FFF8..=0xFFF_FFFF) => break data,
                _ => unimplemented!("{:?} {:#x}", self.type_, fat_entry),
            }
        }
    }

    fn read_fat_entry(&self, cluster: u32) -> u32 {
        match self.type_ {
            Fat12 => unimplemented!(),
            Fat16 => {
                let entries_per_sector = DISK_SECTOR_SIZE as u32 / 2;
                let fat_sectors_start = self.bpb.rsvd_sec_cnt as u64;
                let fat_sector_index = cluster / entries_per_sector;
                let fat_entry_index = cluster % entries_per_sector;
                let sector = self.drive.read(fat_sectors_start + fat_sector_index as u64);
                let entry = sector.as_chunks().0[fat_entry_index as usize];
                u16::from_le_bytes(entry) as u32
            }
            Fat32 => {
                let entries_per_sector = DISK_SECTOR_SIZE as u32 / 4;
                let fat_sectors_start = self.bpb.rsvd_sec_cnt as u64;
                let fat_sector_index = cluster / entries_per_sector;
                let fat_entry_index = cluster % entries_per_sector;
                let sector = self.drive.read(fat_sectors_start + fat_sector_index as u64);
                let entry = sector.as_chunks().0[fat_entry_index as usize];
                u32::from_le_bytes(entry)
            }
        }
    }

    fn read_data(&self, cluster: u32) -> Vec<u8> {
        let cluster_sectors_start =
            self.data_sectors_start() + (cluster - 2) as u64 * self.bpb.sec_per_clus as u64;
        let mut data = Vec::new();
        for i in 0..self.bpb.sec_per_clus as u64 {
            data.extend_from_slice(&self.drive.read(cluster_sectors_start + i));
        }
        data
    }

    fn data_sectors_start(&self) -> u64 {
        self.bpb.rsvd_sec_cnt as u64
            + self.bpb.num_fats as u64 * self.fat_sz as u64
            + self.bpb.root_ent_cnt as u64 * 32 / DISK_SECTOR_SIZE as u64
    }

    fn volume_label(&self) -> ArrayCStr<11> {
        match self.type_ {
            Fat12 | Fat16 => self.bpb.as_extended_12_16().bs_vol_lab,
            Fat32 => self.bpb.as_extended_32().bs_vol_lab,
        }
    }
}

impl FilesystemServer<Directory> for Fat {
    fn read(&mut self, _: &mut Ctx<Self>, dir: Directory, path: &str) -> Vec<u8> {
        let (file, file_size) = self.traverse_path(dir, path);
        let mut data = self.read_file(file);
        data.resize(file_size, 0);
        data
    }

    fn read_large(
        &mut self,
        ctx: &mut Ctx<Self>,
        dir: Directory,
        path: &str,
    ) -> Capability<SharedMemory> {
        let (file, file_size) = self.traverse_path(dir, path);
        let data = self.read_file(file);
        let (shared, shared_cap) = alloc_shared(file_size);
        unsafe { &mut *shared }.copy_from_slice(&data[..file_size]);
        ctx.forward_to_sender(shared_cap)
    }

    fn write(&mut self, _: &mut Ctx<Self>, _dir: Directory, _path: &str, _data: &[u8]) {
        unimplemented!()
    }

    fn subcapability(
        &mut self,
        ctx: &mut Ctx<Self>,
        dir: Directory,
        path: &str,
    ) -> Capability<Filesystem> {
        ctx.grant_to_sender(Directory::Normal {
            cluster: self.traverse_path(dir, path).0,
        })
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
    if common.byts_per_sec as usize != DISK_SECTOR_SIZE {
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
        Fat12
    } else if count_of_clusters < 65525 {
        Fat16
    } else {
        Fat32
    };

    // TODO: Validate the extended BPB.

    if type_ != Fat16 && type_ != Fat32 {
        unimplemented!("unsupported FAT type")
    }

    let server = Fat {
        drive: args.drive,
        bpb,
        fat_sz,
        type_,
        max_cluster: count_of_clusters + 1,
    };

    let volume_label = server.volume_label();
    info!("mounting FAT volume {volume_label:?}");

    let root_directory = match type_ {
        Fat12 | Fat16 => Directory::RootDirectoryRegion,
        Fat32 => Directory::Normal {
            cluster: server.bpb.as_extended_32().root_clus,
        },
    };

    let mut dispatch = Dispatch::new_object(server, root_directory);
    dispatch.run();
}

fn directory_bytes_to_entries(bytes: Vec<u8>) -> Vec<DirectoryEntry> {
    let (ptr, length, capacity) = bytes.into_raw_parts();
    assert_eq!(length % size_of::<DirectoryEntry>(), 0);
    assert_eq!(capacity % size_of::<DirectoryEntry>(), 0);
    unsafe {
        Vec::from_raw_parts(
            ptr as *mut DirectoryEntry,
            length / size_of::<DirectoryEntry>(),
            capacity / size_of::<DirectoryEntry>(),
        )
    }
}

fn str_to_short_file_name(s: &str) -> ArrayCStr<11> {
    let (main_part, extension) = s.split_once('.').unwrap_or((s, ""));
    assert!(main_part.len() <= 8);
    assert!(extension.len() <= 3);
    let mut name = ArrayCStr([b' '; _]);
    for (i, byte) in main_part.bytes().enumerate() {
        let byte = byte.to_ascii_uppercase();
        assert!(is_valid_short_file_name_char(byte));
        name.0[i] = byte;
    }
    for (i, byte) in extension.bytes().enumerate() {
        let byte = byte.to_ascii_uppercase();
        assert!(is_valid_short_file_name_char(byte));
        name.0[8 + i] = byte;
    }
    name
}

fn is_valid_short_file_name_char(byte: u8) -> bool {
    matches!(byte, b'A'..=b'Z' | b'0'..=b'9' | 128.. | b'$' | b'%' | b'\'' | b'-' | b'_' | b'@' | b'~' | b'`' | b'!' | b'(' | b')' | b'{' | b'}' | b'^' | b'#' | b'&')
}

app! { main }
