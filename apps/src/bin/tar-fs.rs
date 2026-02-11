#![allow(static_mut_refs)]
#![no_std]
#![no_main]

extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::ffi::CStr;
use deravel_kernel_api::*;
use log::debug;

struct File {
    name: String,
    data: Vec<u8>,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct TarHeader {
    name: [u8; 100],
    mode: [u8; 8],
    uid: [u8; 8],
    gid: [u8; 8],
    size: [u8; 12],
    mtime: [u8; 12],
    checksum: [u8; 8],
    type_: u8,
    linkname: [u8; 100],
    magic: [u8; 6],
    version: [u8; 2],
    uname: [u8; 32],
    gname: [u8; 32],
    devmajor: [u8; 8],
    devminor: [u8; 8],
    prefix: [u8; 155],
    padding: [u8; 12],
}

union TarHeaderBuf {
    header: TarHeader,
    bytes: [u8; 512],
}

const SECTOR_SIZE: usize = 512;

fn main() {
    let files = parse_tar_file();
    for file in files {
        let name = file.name;
        let contents = str::from_utf8(&file.data).unwrap();
        debug!("read file {name} with contents {contents:?}");
    }
}

fn parse_tar_file() -> Vec<File> {
    let capacity = disk_capacity();
    let mut sector = 0;
    let mut buf = TarHeaderBuf {
        bytes: [0; SECTOR_SIZE],
    };
    let mut files = Vec::new();
    while sector < capacity {
        // Hopefully one day Rust will get safe transmute and something this simple will
        // become nice and safe. The alternative is error-prone indexing with prefix
        // sums or a macro.
        disk_read(sector, unsafe { &mut buf.bytes });
        sector += 1;
        let header = unsafe { &buf.header };

        // TODO: Shouldn't it be two consecutive zero-filled blocks?
        if header.name[0] == b'\0' {
            break;
        }

        assert_eq!(header.magic, *b"ustar\0");

        // TODO: Check it's a valid UNIX path with no weird components.
        let name = parse_string(&header.name).to_owned();

        let size = parse_octal(&header.size);
        assert!(sector + size.div_ceil(SECTOR_SIZE) - 1 < capacity);

        let mut data = vec![0u8; size.next_multiple_of(SECTOR_SIZE)];
        for i in 0..size.div_ceil(SECTOR_SIZE) {
            let sector_data = &mut data[i * SECTOR_SIZE..][..SECTOR_SIZE];
            disk_read(sector, sector_data.try_into().unwrap());
            sector += 1;
        }
        data.resize(size, 0);

        files.push(File { name, data });
    }
    files
}

fn parse_string(raw: &[u8]) -> &str {
    CStr::from_bytes_until_nul(raw).unwrap().to_str().unwrap()
}

fn parse_octal(raw: &[u8]) -> usize {
    usize::from_str_radix(parse_string(raw), 8).unwrap()
}

app! { main }
