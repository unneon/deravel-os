#![no_std]
#![no_main]
extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::vec::Vec;
use deravel_interfaces::FilesystemRequest;
use deravel_kernel_api::*;
use log::{debug, trace};

fn main() {
    let fs = pid_by_name("fs-tar");

    let (cap, _) = ipc_recv::<Capability>();

    trace!("demonstrating {cap:?} to {fs:?}");
    ipc_send(
        &FilesystemRequest::Read {
            cap,
            path: "secret.txt".to_owned(),
        },
        fs,
    );
    let (data, _) = ipc_recv::<Vec<u8>>();
    let text = core::str::from_utf8(&data).unwrap();
    debug!("read {text:?} from file");
}

app! { main }
