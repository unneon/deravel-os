#![allow(static_mut_refs)]
#![no_std]
#![no_main]

extern crate alloc;

mod deserialize;
mod sectors;
mod serialize;

use crate::deserialize::deserialize_archive;
use crate::serialize::serialize_archive;
use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::{format, vec};
use deravel_interfaces::FilesystemRequest;
use deravel_kernel_api::*;
use log::error;

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
    let mut files = deserialize_archive();

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
                let path = concat_path(path_prefix, &path_suffix);
                let file = files.iter().find(|file| file.name == path);
                if let Some(file) = file {
                    ipc_send(&file.data[..file.size], req_sender);
                } else {
                    error!("file {path:?} not found");
                }
            }
            FilesystemRequest::Write {
                cap,
                path: path_suffix,
                mut data,
            } => {
                let cap = &capabilities[cap.validate(req_sender).local_index()];
                let path_prefix = &cap.path;
                let path = concat_path(path_prefix, &path_suffix);
                let file = files.iter().find(|file| file.name == path);
                if file.is_none() {
                    let size = data.len();
                    data.resize(size.next_multiple_of(SECTOR_SIZE), 0);
                    files.push(File {
                        name: path.into_owned(),
                        data,
                        size,
                    });
                } else {
                    error!("file {path:?} already exists");
                }
            }
            FilesystemRequest::Subcapability {
                cap,
                path: path_suffix,
            } => {
                let cap = &capabilities[cap.validate(req_sender).local_index()];
                let path_prefix = &cap.path;
                let path = concat_path(path_prefix, &path_suffix).into_owned();
                capabilities.push(CapabilityData { path });
                let sub_cap = Capability::grant(req_sender);
                ipc_send(&sub_cap, req_sender);
            }
        }
        serialize_archive(&files);
    }
}

fn concat_path<'a>(prefix: &'a str, suffix: &'a str) -> Cow<'a, str> {
    if prefix.is_empty() {
        suffix.into()
    } else {
        format!("{prefix}/{suffix}").into()
    }
}

app! { main }
