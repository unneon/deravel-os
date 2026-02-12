#![no_std]
#![no_main]
extern crate alloc;

use alloc::borrow::ToOwned;
use deravel_interfaces::FilesystemRequest;
use deravel_kernel_api::*;

fn main() {
    let b = pid_by_name("ipc-b");

    let (fs_root_cap, fs) = ipc_recv::<Capability>();
    ipc_send(
        &FilesystemRequest::Read {
            cap: fs_root_cap,
            path: "hello.txt".to_owned(),
        },
        fs,
    );
    ipc_send(
        &FilesystemRequest::Subcapability {
            cap: fs_root_cap,
            path: "user/".to_owned(),
        },
        fs,
    );
    let (fs_user_cap, _) = ipc_recv::<Capability>();

    let cap = fs_user_cap.forward(b);
    ipc_send(&cap, b);
}

app! { main }
