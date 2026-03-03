#![no_std]
#![no_main]

use deravel_kernel_api::*;

fn main(caps: Capabilities) {
    let (fs, _) = cap_recv::<Capability>();
    let fs_forwarded = forward_capability(fs, caps.c.into());
    caps.c.bar(fs_forwarded);
}

app! { main ipc_b_prelude }
