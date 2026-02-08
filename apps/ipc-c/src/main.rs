#![no_std]
#![no_main]

use deravel_kernel_api::*;

fn main() {
    let a = pid_by_name("ipc-a");

    let cap: Capability = ipc_recv().0;

    ipc_send(&cap, a);
}

app! { main }
