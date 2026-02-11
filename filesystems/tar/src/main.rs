#![allow(static_mut_refs)]
#![no_std]
#![no_main]

extern crate alloc;

mod deserialize;
mod sectors;
mod serialize;

use crate::deserialize::deserialize_archive;
use crate::serialize::serialize_archive;
use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use deravel_kernel_api::*;
use log::debug;

struct File {
    name: String,
    data: Vec<u8>,
    size: usize,
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
    bytes: [u8; SECTOR_SIZE],
}

const SECTOR_SIZE: usize = 512;

fn main() {
    let mut files = deserialize_archive();
    for file in &files {
        let name = &file.name;
        let contents = str::from_utf8(&file.data[..file.size]).unwrap();
        debug!("read file {name} with contents {contents:?}");
    }
    files.push(File {
        name: "debug.log".to_owned(),
        data: "no crash detected\n".as_bytes().to_owned(),
        size: "no crash detected\n".len(),
    });
    files.last_mut().unwrap().data.resize(SECTOR_SIZE, b' ');
    serialize_archive(&files);
}

app! { main }
