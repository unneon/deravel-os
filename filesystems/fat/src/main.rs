#![feature(maybe_uninit_fill)]
#![feature(min_adt_const_params)]
#![no_std]
#![no_main]

extern crate alloc;

mod bpb;
mod directory_entry;
mod util;

use crate::Type::*;
use crate::bpb::Bpb;
use crate::directory_entry::{DirectoryEntry, coalesce_long_names, to_short_name};
use crate::util::ArrayCStr;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::ConstParamTy;
use deravel_kernel_api::*;
use log::*;

#[derive(Clone, Copy, Debug)]
enum Directory {
    Normal { cluster: u32 },
    RootDirectoryRegion,
}

struct Fat<const TYPE: Type> {
    drive: Capability<Drive>,
    bpb: Bpb,
    max_cluster: u32,
}

#[derive(Clone, ConstParamTy, Copy, Debug, Eq, PartialEq)]
enum Type {
    Fat12,
    Fat16,
    Fat32,
}

impl<const TYPE: Type> Fat<TYPE> {
    fn traverse_path(&self, mut dir: Directory, mut path: &str) -> (u32, usize) {
        'segments: loop {
            let (path_seg, path_tail) = match path.split_once('/') {
                Some((path_seg, path_tail)) => (path_seg, Some(path_tail)),
                None => (path, None),
            };
            let short_needle = to_short_name(path_seg);
            for (de, long_name) in coalesce_long_names(self.read_directory(dir).into_iter()) {
                if de.name.0[0] == 0xE5 {
                    continue;
                }
                if de.name.0[0] == 0x00 {
                    break;
                }
                let name_matches = if let Some(long_name) = long_name {
                    long_name == path_seg
                } else {
                    short_needle == Some(de.name)
                };
                if name_matches {
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
        let rdr_sectors_count = self.bpb.root_ent_cnt as u64 * 32 / self.bpb.byts_per_sec as u64;
        let mut entries = Vec::new();
        for i in 0..rdr_sectors_count {
            let bytes = self.read_sector(self.root_directory_sectors_start() + i);
            entries.extend_from_slice(&directory_bytes_to_entries(bytes));
        }
        entries
    }

    fn read_file(&self, mut cluster: u32) -> Vec<u8> {
        let mut data = Vec::new();
        loop {
            data.extend_from_slice(&self.read_data(cluster));
            let fat_entry = self.read_fat_entry(cluster);
            match (TYPE, fat_entry & ((!0) >> 4)) {
                (Fat12, next_cluster @ 0x002..)
                | (Fat16, next_cluster @ 0x0002..)
                | (Fat32, next_cluster @ 0x000_0002..)
                    if next_cluster <= self.max_cluster =>
                {
                    cluster = next_cluster;
                }
                (Fat12, 0xFF8..=0xFFF)
                | (Fat16, 0xFFF8..=0xFFFF)
                | (Fat32, 0xFFF_FFF8..=0xFFF_FFFF) => break data,
                _ => unimplemented!("{:?} {:#x}", TYPE, fat_entry),
            }
        }
    }

    fn read_fat_entry(&self, cluster: u32) -> u32 {
        match TYPE {
            Fat12 => {
                let global_byte_index = cluster + cluster / 2;
                let fat_sector_index = global_byte_index as u64 / self.bpb.byts_per_sec as u64;
                let fat_entry_byte_index =
                    global_byte_index as usize % self.bpb.byts_per_sec as usize;
                let bytes = if fat_entry_byte_index + 1 < self.bpb.byts_per_sec as usize {
                    let sector = self.read_sector(self.fat_sectors_start() + fat_sector_index);
                    [
                        sector[fat_entry_byte_index],
                        sector[fat_entry_byte_index + 1],
                    ]
                } else {
                    let first_sector =
                        self.read_sector(self.fat_sectors_start() + fat_sector_index);
                    let second_sector = self
                        .drive
                        .read(self.fat_sectors_start() + fat_sector_index + 1);
                    [first_sector[fat_entry_byte_index], second_sector[0]]
                };
                let value = u16::from_le_bytes(bytes) as u32;
                if cluster.is_multiple_of(2) {
                    value & 0x0FFF
                } else {
                    value >> 4
                }
            }
            Fat16 => {
                let entries_per_sector = self.bpb.byts_per_sec as u32 / 2;
                let fat_sector_index = (cluster / entries_per_sector) as u64;
                let fat_entry_index = cluster % entries_per_sector;
                let sector = self.read_sector(self.fat_sectors_start() + fat_sector_index);
                let entry = sector.as_chunks().0[fat_entry_index as usize];
                u16::from_le_bytes(entry) as u32
            }
            Fat32 => {
                let entries_per_sector = self.bpb.byts_per_sec as u32 / 4;
                let fat_sector_index = (cluster / entries_per_sector) as u64;
                let fat_entry_index = cluster % entries_per_sector;
                let sector = self.read_sector(self.fat_sectors_start() + fat_sector_index);
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
            data.extend_from_slice(&self.read_sector(cluster_sectors_start + i));
        }
        data
    }

    fn read_sector(&self, sector: u64) -> Vec<u8> {
        const DISK_SECTOR_SIZE: usize = 512;
        let disk_sector_per_fat_sector = self.bpb.byts_per_sec as u64 / DISK_SECTOR_SIZE as u64;
        let mut bytes = Vec::with_capacity(self.bpb.byts_per_sec as usize);
        for i in 0..disk_sector_per_fat_sector {
            bytes.extend_from_slice(&self.drive.read(disk_sector_per_fat_sector * sector + i));
        }
        bytes
    }

    fn fat_sectors_size(&self) -> u32 {
        match TYPE {
            Fat12 | Fat16 => self.bpb.fat_sz_16 as u32,
            Fat32 => self.bpb.as_extended_32().fat_sz_32,
        }
    }

    fn fat_sectors_start(&self) -> u64 {
        self.bpb.rsvd_sec_cnt as u64
    }

    fn root_directory_sectors_start(&self) -> u64 {
        self.fat_sectors_start() + self.bpb.num_fats as u64 * self.fat_sectors_size() as u64
    }

    fn data_sectors_start(&self) -> u64 {
        self.root_directory_sectors_start()
            + self.bpb.root_ent_cnt as u64 * 32 / self.bpb.byts_per_sec as u64
    }

    fn volume_label(&self) -> Option<ArrayCStr<11>> {
        let name = match TYPE {
            Fat12 | Fat16 => self.bpb.as_extended_12_16().bs_vol_lab,
            Fat32 => self.bpb.as_extended_32().bs_vol_lab,
        };
        if &name.0 == b"NO NAME    " {
            return None;
        };
        Some(name)
    }
}

impl<const TYPE: Type> FilesystemServer<Directory> for Fat<TYPE> {
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
    let (type_, count_of_clusters) = determine_type(&bpb);
    match type_ {
        Fat12 => run::<{ Fat12 }>(args.drive, bpb, count_of_clusters),
        Fat16 => run::<{ Fat16 }>(args.drive, bpb, count_of_clusters),
        Fat32 => run::<{ Fat32 }>(args.drive, bpb, count_of_clusters),
    }
}

fn run<const TYPE: Type>(drive: Capability<Drive>, bpb: Bpb, count_of_clusters: u32) {
    bpb.validate_bpb(TYPE);
    let server = Fat::<{ TYPE }> {
        drive,
        bpb,
        max_cluster: count_of_clusters + 1,
    };

    if let Some(volume_label) = server.volume_label() {
        info!("mounting FAT volume {volume_label:?}");
    } else {
        info!("mounting unnamed FAT volume");
    }

    let root_directory = match TYPE {
        Fat12 | Fat16 => Directory::RootDirectoryRegion,
        Fat32 => Directory::Normal {
            cluster: server.bpb.as_extended_32().root_clus,
        },
    };

    let mut dispatch = Dispatch::new_object(server, root_directory);
    dispatch.run();
}

fn determine_type(bpb: &Bpb) -> (Type, u32) {
    let root_dir_sectors = (bpb.root_ent_cnt as u32 * 32).div_ceil(bpb.byts_per_sec as u32);
    let fat_sz = if bpb.fat_sz_16 != 0 {
        bpb.fat_sz_16 as u32
    } else {
        bpb.as_extended_32().fat_sz_32
    };

    let tot_sec = if bpb.tot_sec_16 != 0 {
        bpb.tot_sec_16 as u32
    } else {
        bpb.tot_sec_32
    };

    let data_sec =
        tot_sec - (bpb.rsvd_sec_cnt as u32 + bpb.num_fats as u32 * fat_sz) + root_dir_sectors;

    let count_of_clusters = data_sec / bpb.sec_per_clus as u32;

    let type_ = if count_of_clusters < 4085 {
        Fat12
    } else if count_of_clusters < 65525 {
        Fat16
    } else {
        Fat32
    };
    (type_, count_of_clusters)
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

app! { main }
