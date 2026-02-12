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

#[derive(Debug)]
struct CapabilityData {
    path: String,
}

struct File {
    name: String,
    data: Vec<u8>,
    size: usize,
}

#[derive(Debug)]
struct Request {
    capability: Capability,
    type_: usize,
    text: [u8; 16],
    text_len: usize,
    data: [u8; 16],
    data_len: usize,
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

    let mut capabilities = Vec::new();
    capabilities.push(CapabilityData {
        path: String::new(),
    });
    let ipc_a = pid_by_name("ipc-a");
    let root_cap = Capability::grant(ipc_a);
    ipc_send(&root_cap, ipc_a);

    loop {
        let (req, req_sender) = ipc_recv::<Request>();
        let cap = req.capability.validate(req_sender);
        let cap = &capabilities[cap.local_index()];
        let path_prefix = &cap.path;
        let path_suffix = core::str::from_utf8(&req.text[..req.text_len]).unwrap();
        if req.type_ == 0 {
            debug!("received read request of {path_prefix:?} {path_suffix:?}");
        } else if req.type_ == 1 {
            debug!("received write request of {path_prefix:?} {path_suffix:?}");
        } else {
            debug!("received subcapability request");
        }
        serialize_archive(&files);
    }
}

app! { main }
