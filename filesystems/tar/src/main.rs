#![allow(static_mut_refs)]
#![no_std]
#![no_main]

extern crate alloc;

mod deserialize;
mod sectors;
mod serialize;

use crate::deserialize::deserialize_archive;
use crate::serialize::serialize_archive;
use alloc::borrow::{Cow, ToOwned};
use alloc::string::String;
use alloc::vec::Vec;
use alloc::{format, vec};
use deravel_kernel_api::deravel_types::capability::Capability;
use deravel_kernel_api::drvli::{FilesystemServer, ipc_serve_filesystem};
use deravel_kernel_api::*;
use deravel_types::ProcessId;
use deravel_types::drvli::Filesystem;
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

struct Server {
    files: Vec<File>,
    capabilities: Vec<CapabilityData>,
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

impl FilesystemServer for Server {
    fn read(&mut self, cap: RawCapability, _: ProcessId, path_suffix: &str) -> Vec<u8> {
        let cap = &self.capabilities[cap.local_index()];
        let path_prefix = &cap.path;
        let path = concat_path(path_prefix, path_suffix);
        let file = self.files.iter().find(|file| file.name == path);
        let Some(file) = file else {
            panic!("file {path:?} not found");
        };
        file.data[..file.size].to_owned()
    }

    fn write(&mut self, cap: RawCapability, _: ProcessId, path_suffix: &str, data: &[u8]) {
        let cap = &self.capabilities[cap.local_index()];
        let path_prefix = &cap.path;
        let path = concat_path(path_prefix, path_suffix);
        let file = self.files.iter().find(|file| file.name == path);
        if file.is_some() {
            error!("file {path:?} already exists");
        }
        let size = data.len();
        let mut data = data.to_owned();
        data.resize(size.next_multiple_of(SECTOR_SIZE), 0);
        self.files.push(File {
            name: path.into_owned(),
            data,
            size,
        });
        serialize_archive(&self.files);
    }

    fn subcapability(
        &mut self,
        cap: RawCapability,
        sender: ProcessId,
        path_suffix: &str,
    ) -> Capability<Filesystem> {
        let cap = &self.capabilities[cap.local_index()];
        let path_prefix = &cap.path;
        let path = concat_path(path_prefix, path_suffix).into_owned();
        self.capabilities.push(CapabilityData { path });
        grant_capability(sender)
    }
}

fn main(_: Args) {
    let files = deserialize_archive();
    let capabilities = vec![CapabilityData {
        path: String::new(),
    }];
    ipc_serve_filesystem(Server {
        files,
        capabilities,
    })
}

fn concat_path<'a>(prefix: &'a str, suffix: &'a str) -> Cow<'a, str> {
    if prefix.is_empty() {
        suffix.into()
    } else {
        format!("{prefix}/{suffix}").into()
    }
}

app! { main TarFs }
