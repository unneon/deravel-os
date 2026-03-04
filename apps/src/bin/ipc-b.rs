#![no_std]
#![no_main]

use deravel_kernel_api::*;

fn main(Args { c }: Args) {
    let (fs, _) = cap_recv::<Capability>();
    let fs_forwarded = forward_capability(fs, c.into());
    c.bar(fs_forwarded);
}

app! { main ipc_b }
