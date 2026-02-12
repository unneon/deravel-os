#![no_std]
#![no_main]
extern crate alloc;

use alloc::borrow::ToOwned;
use deravel_interfaces::FilesystemRequest;
use deravel_kernel_api::*;
use log::trace;

fn main() {
    let fs = pid_by_name("fs-tar");

    let (cap, _) = ipc_recv::<Capability>();

    trace!("demonstrating {cap:?} to {fs:?}");
    ipc_send(
        &FilesystemRequest::Read {
            cap,
            path: "hello.txt".to_owned(),
        },
        fs,
    );

    // ipc_send(
    //     &FilesystemRequest::Read {
    //         cap: unsafe { core::mem::transmute::<usize, Capability>(0x2000000) },
    //         path: "hello.txt".to_owned(),
    //     },
    //     fs,
    // );
}

app! { main }
