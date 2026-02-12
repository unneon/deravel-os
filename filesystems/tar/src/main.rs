#![allow(static_mut_refs)]
#![no_std]
#![no_main]

extern crate alloc;

mod deserialize;
mod sectors;
mod serialize;

use crate::deserialize::deserialize_archive;
use crate::serialize::serialize_archive;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::{format, vec};
use deravel_interfaces::FilesystemRequest;
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
    let files = deserialize_archive();

    let mut capabilities = vec![CapabilityData {
        path: String::new(),
    }];
    let ipc_a = pid_by_name("ipc-a");
    let root_cap = Capability::grant(ipc_a);
    ipc_send(&root_cap, ipc_a);

    loop {
        let (req, req_sender) = ipc_recv::<FilesystemRequest>();
        match req {
            FilesystemRequest::Read {
                cap,
                path: path_suffix,
            } => {
                let cap = &capabilities[cap.validate(req_sender).local_index()];
                let path_prefix = &cap.path;
                debug!("received read request of {path_prefix:?} {path_suffix:?}");
            }
            FilesystemRequest::Write {
                cap,
                path: path_suffix,
                data: _data,
            } => {
                let cap = &capabilities[cap.validate(req_sender).local_index()];
                let path_prefix = &cap.path;
                debug!("received write request of {path_prefix:?} {path_suffix:?}");
            }
            FilesystemRequest::Subcapability {
                cap,
                path: path_suffix,
            } => {
                let cap = &capabilities[cap.validate(req_sender).local_index()];
                let path_prefix = &cap.path;
                debug!("received subcapability request of {path_prefix:?} {path_suffix:?}");
                capabilities.push(CapabilityData {
                    path: format!("{path_prefix}{path_suffix}"),
                });
                let sub_cap = Capability::grant(req_sender);
                ipc_send(&sub_cap, req_sender);
            }
        }
        serialize_archive(&files);
    }
}

app! { main }
