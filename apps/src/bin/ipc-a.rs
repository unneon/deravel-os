#![no_std]
#![no_main]

use deravel_kernel_api::drvli::{FilesystemClient, IpcBClient};
use deravel_kernel_api::*;

fn main(Args { fs, b }: Args) {
    fs.write("secret.txt", b"admin secret");
    fs.write("user/secret.txt", b"user secret");
    let user = fs.subcapability("user");
    let user = forward_capability(user, b);
    b.foo(user);
}

app! { main IpcA }
