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
use deravel_kernel_api::*;
use log::error;

struct File {
    name: String,
    data: Vec<u8>,
    size: usize,
}

struct Server {
    drive: Capability<Drive>,
    files: Vec<File>,
    caps: Vec<String>,
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

impl FilesystemServer<usize> for Server {
    fn read(&mut self, _: &mut Ctx<Self>, cap: usize, path_suffix: &str) -> Vec<u8> {
        let path = concat_path(&self.caps[cap], path_suffix);
        let file = self.files.iter().find(|file| file.name == path);
        let Some(file) = file else {
            panic!("file {path:?} not found")
        };
        file.data[..file.size].to_owned()
    }

    fn write(&mut self, _: &mut Ctx<Self>, cap: usize, path_suffix: &str, data: &[u8]) {
        let path = concat_path(&self.caps[cap], path_suffix);
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
        serialize_archive(&self.files, self.drive);
    }

    fn subcapability(
        &mut self,
        ctx: &mut Ctx<Self>,
        cap: usize,
        path_suffix: &str,
    ) -> Capability<Filesystem> {
        let path = concat_path(&self.caps[cap], path_suffix);
        let new_cap = self.caps.len();
        self.caps.push(path.into_owned());
        ctx.grant_capability(new_cap)
    }
}

fn main(args: Args) {
    let files = deserialize_archive(args.drive);
    let server = Server {
        drive: args.drive,
        files,
        caps: vec![String::new()],
    };
    let mut dispatch = Dispatch::new_object(server, 0);
    loop {
        ipc_serve(&mut dispatch);
        yield_();
    }
}

fn concat_path<'a>(prefix: &'a str, suffix: &'a str) -> Cow<'a, str> {
    if prefix.is_empty() {
        suffix.into()
    } else {
        format!("{prefix}/{suffix}").into()
    }
}

app! { main TarFs }
