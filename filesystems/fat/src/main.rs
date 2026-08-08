#![feature(exact_div)]
#![feature(min_adt_const_params)]
#![no_std]
#![no_main]

extern crate alloc;

mod bpb;
mod directory;

use crate::Type::*;
use crate::bpb::Bpb;
use crate::directory::{DirectoryEntry, coalesce_long_names, to_short_name};
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::ConstParamTy;
use core::ops::Range;
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
    fat: &'static PageAligned<[u8]>,
    rdr: &'static [DirectoryEntry],
}

#[derive(Clone, ConstParamTy, Copy, Debug, Eq, PartialEq)]
enum Type {
    Fat12,
    Fat16,
    Fat32,
}

const DISK_SECTOR_SIZE: usize = 512;

impl<const TYPE: Type> Fat<TYPE> {
    fn traverse_path(&self, mut dir: Directory, mut path: &str) -> (u32, u32) {
        'segments: loop {
            let (path_seg, path_tail) = match path.split_once('/') {
                Some((path_seg, path_tail)) => (path_seg, Some(path_tail)),
                None => (path, None),
            };
            let short_needle = to_short_name(path_seg);
            for (de, long_name) in coalesce_long_names(&self.read_directory(dir)) {
                if de.name[0] == 0xE5 {
                    continue;
                }
                if de.name[0] == 0x00 {
                    break;
                }
                let name_matches = if let Some(long_name) = long_name {
                    long_name == path_seg
                } else {
                    short_needle == Some(de.name)
                };
                if name_matches {
                    let de_cluster = de.fst_clus();
                    let Some(path_tail) = path_tail else {
                        return (de_cluster, de.file_size);
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

    fn read_directory(&self, dir: Directory) -> Cow<'_, [DirectoryEntry]> {
        match dir {
            Directory::Normal { cluster } => Cow::Owned(self.read_normal_directory(cluster)),
            Directory::RootDirectoryRegion => Cow::Borrowed(self.read_root_directory_region()),
        }
    }

    fn read_normal_directory(&self, cluster: u32) -> Vec<DirectoryEntry> {
        directory_bytes_to_entries(self.read_file(cluster, None))
    }

    fn read_root_directory_region(&self) -> &[DirectoryEntry] {
        self.rdr
    }

    fn read_file(&self, cluster: u32, size: Option<usize>) -> Vec<u8> {
        let mut data = Vec::new();
        if let Some(size) = size {
            data.reserve_exact(size.next_multiple_of(DISK_SECTOR_SIZE));
        }
        'walk: for cluster in self.walk_clusters(cluster) {
            for sector in self.sectors_of_cluster(cluster) {
                for disk_sector in self.drive_sectors_of_sector(sector) {
                    data.extend_from_slice(&self.drive.read(disk_sector));
                    if let Some(size) = size
                        && data.len() >= size
                    {
                        break 'walk;
                    }
                }
            }
        }
        if let Some(size) = size {
            data.resize(size, 0);
        }
        data
    }

    fn walk_clusters(&self, cluster: u32) -> impl Iterator<Item = u32> {
        core::iter::successors(Some(cluster), move |&cluster| {
            let fat_entry = self.read_fat_entry(cluster);
            match (TYPE, fat_entry & ((!0) >> 4)) {
                (Fat12, next_cluster @ 0x002..)
                | (Fat16, next_cluster @ 0x0002..)
                | (Fat32, next_cluster @ 0x000_0002..)
                    if next_cluster <= self.max_cluster() =>
                {
                    Some(next_cluster)
                }
                (Fat12, 0xFF8..=0xFFF)
                | (Fat16, 0xFFF8..=0xFFFF)
                | (Fat32, 0xFFF_FFF8..=0xFFF_FFFF) => None,
                _ => unimplemented!("{:?} {:#x}", TYPE, fat_entry),
            }
        })
    }

    fn read_fat_entry(&self, cluster: u32) -> u32 {
        match TYPE {
            Fat12 => {
                let global_byte_index = cluster + cluster / 2;
                let bytes: &[u8; 2] = self.fat.0[global_byte_index as usize..][..2]
                    .try_into()
                    .unwrap();
                let value = u16::from_le_bytes(*bytes) as u32;
                if cluster.is_multiple_of(2) {
                    value & 0x0FFF
                } else {
                    value >> 4
                }
            }
            Fat16 => self.fat_16()[cluster as usize] as u32,
            Fat32 => self.fat_32()[cluster as usize],
        }
    }

    fn fat_16(&self) -> &'static [u16] {
        unsafe { &*PageAligned::cast(self.fat) }
    }

    fn fat_32(&self) -> &'static [u32] {
        unsafe { &*PageAligned::cast(self.fat) }
    }

    fn sectors_of_cluster(&self, cluster: u32) -> impl Iterator<Item = u32> {
        (0..self.bpb.sec_per_clus as u32).map(move |i| {
            self.data_sectors().start + (cluster - 2) * self.bpb.sec_per_clus as u32 + i
        })
    }

    fn drive_sectors_of_sector(&self, sector: u32) -> impl Iterator<Item = u64> {
        let ratio = self.bpb.byts_per_sec as u64 / DISK_SECTOR_SIZE as u64;
        (0..ratio).map(move |i| ratio * sector as u64 + i)
    }

    fn fat_sectors(&self) -> Range<u32> {
        let start = self.bpb.rsvd_sec_cnt as u32;
        let single_size = match TYPE {
            Fat12 | Fat16 => self.bpb.fat_sz_16 as u32,
            Fat32 => self.bpb.as_extended_32().fat_sz_32,
        };
        let total_size = self.bpb.num_fats as u32 * single_size;
        start..start + total_size
    }

    fn root_directory_sectors(&self) -> Range<u32> {
        let start = self.fat_sectors().end;
        let size = self.bpb.root_ent_cnt as u32 * 32 / self.bpb.byts_per_sec as u32;
        start..start + size
    }

    fn data_sectors(&self) -> Range<u32> {
        let start = self.root_directory_sectors().end;
        let end = self.total_sectors_count();
        start..end
    }

    fn total_sectors_count(&self) -> u32 {
        match TYPE {
            Fat12 | Fat16 => {
                if self.bpb.tot_sec_16 != 0 {
                    self.bpb.tot_sec_16 as u32
                } else {
                    self.bpb.tot_sec_32
                }
            }
            Fat32 => self.bpb.tot_sec_32,
        }
    }

    fn count_of_clusters(&self) -> u32 {
        (self.total_sectors_count() - self.data_sectors().start) / self.bpb.sec_per_clus as u32
    }

    fn max_cluster(&self) -> u32 {
        self.count_of_clusters() + 1
    }

    fn volume_label(&self) -> Option<[u8; 11]> {
        let name = match TYPE {
            Fat12 | Fat16 => self.bpb.as_extended_12_16().bs_vol_lab,
            Fat32 => self.bpb.as_extended_32().bs_vol_lab,
        };
        if &name == b"NO NAME    " {
            return None;
        };
        Some(name)
    }
}

impl<const TYPE: Type> FilesystemServer<Directory> for Fat<TYPE> {
    fn read(&mut self, _: &mut Ctx<Self>, dir: Directory, path: &str) -> Vec<u8> {
        let (file, file_size) = self.traverse_path(dir, path);
        self.read_file(file, Some(file_size as usize))
    }

    fn read_large(
        &mut self,
        ctx: &mut Ctx<Self>,
        dir: Directory,
        path: &str,
    ) -> Capability<SharedMemory> {
        let (file, file_size) = self.traverse_path(dir, path);
        let data = self.read_file(file, Some(file_size as usize));
        let (shared, shared_cap) = alloc_shared(file_size as usize);
        unsafe { &mut (*shared).0 }.copy_from_slice(&data);
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

fn main(args: FatFsArgs) {
    let bpb = Bpb {
        bytes: *Box::try_from(args.drive.read(0)).unwrap(),
    };
    let type_ = bpb.determine_type();
    match type_ {
        Fat12 => run::<{ Fat12 }>(args.drive, bpb),
        Fat16 => run::<{ Fat16 }>(args.drive, bpb),
        Fat32 => run::<{ Fat32 }>(args.drive, bpb),
    }
}

fn run<const TYPE: Type>(drive: Capability<Drive>, bpb: Bpb) {
    let mut server = Fat::<{ TYPE }> {
        drive,
        bpb,
        fat: &PageAligned([]),
        rdr: &[],
    };

    let disk_sector_offset = (server.fat_sectors().start as u64 * server.bpb.byts_per_sec as u64)
        .div_exact(DISK_SECTOR_SIZE as u64)
        .unwrap();
    let disk_sector_count = ((server.fat_sectors().end - server.fat_sectors().start) as u64
        * server.bpb.byts_per_sec as u64)
        .div_exact(DISK_SECTOR_SIZE as u64)
        .unwrap();
    let fat = map_shared(drive.read_mapped(disk_sector_offset, disk_sector_count));
    server.fat = unsafe { &*fat };

    if TYPE == Fat12 || TYPE == Fat16 {
        let disk_sector_offset = (server.root_directory_sectors().start as u64
            * server.bpb.byts_per_sec as u64)
            .div_exact(DISK_SECTOR_SIZE as u64)
            .unwrap();
        let disk_sector_count = ((server.root_directory_sectors().end
            - server.root_directory_sectors().start) as u64
            * server.bpb.byts_per_sec as u64)
            .div_exact(DISK_SECTOR_SIZE as u64)
            .unwrap();
        let rdr = map_shared(drive.read_mapped(disk_sector_offset, disk_sector_count));
        server.rdr = unsafe { &*PageAligned::cast(rdr) };
    }

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
