#![allow(static_mut_refs)]
#![no_std]
#![no_main]

use core::ffi::CStr;
use deravel_kernel_api::*;
use log::debug;

#[repr(C, packed)]
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

#[derive(Clone, Copy)]
struct File {
    in_use: bool,
    name: [u8; 100],
    data: [u8; 1024],
    size: usize,
}

const MAX_DISK_SECTORS: usize = 64;
const MAX_FILES: usize = 2;

static mut FILES: [File; MAX_FILES] = [File {
    in_use: false,
    name: [0; 100],
    data: [0; 1024],
    size: 0,
}; MAX_FILES];
static mut DISK: [[u8; 512]; MAX_DISK_SECTORS] = [[0; 512]; MAX_DISK_SECTORS];

fn main() {
    let capacity = disk_capacity();
    assert!(capacity <= MAX_DISK_SECTORS);

    for (sector, buf) in unsafe { DISK[..capacity].iter_mut().enumerate() } {
        disk_read(sector, buf);
    }

    let mut offset = 0;
    for file in unsafe { FILES.iter_mut() } {
        let header = unsafe { &*((&raw const DISK[offset]) as *const TarHeader) };
        if header.name[0] == b'\0' {
            break;
        }

        assert_eq!(header.magic, *b"ustar\0");

        let name = CStr::from_bytes_until_nul(&header.name)
            .unwrap()
            .to_str()
            .unwrap();

        let file_size = parse_octal(&header.size);

        let data = unsafe {
            core::slice::from_raw_parts(&raw const DISK[offset + 1] as *const u8, file_size)
        };

        file.in_use = true;
        file.name.copy_from_slice(&header.name);
        file.data[..data.len()].copy_from_slice(data);
        file.size = file_size;

        debug!(
            "file {name} read from disk {:?}",
            core::str::from_utf8(&file.data[..data.len()]).unwrap()
        );

        offset += file_size.div_ceil(512) + 1;
    }
}

fn parse_octal(raw: &[u8]) -> usize {
    let text = CStr::from_bytes_until_nul(raw).unwrap().to_str().unwrap();
    usize::from_str_radix(text, 8).unwrap()
}

app! { main }
