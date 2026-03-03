#![no_std]
#![no_main]
extern crate alloc;

use deravel_kernel_api::*;

fn main(caps: Capabilities) {
    caps.fs.write("secret.txt", b"admin secret");
    caps.fs.write("user/secret.txt", b"user secret");
    let user = caps.fs.subcapability("user");
    let user = forward_capability(user, caps.b.into());
    caps.b.foo(user);
}

app! { main ipc_a_prelude }
