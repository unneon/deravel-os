#![no_std]
#![no_main]

use deravel_kernel_api::*;

fn main() {
    let c = pid_by_name("ipc-c");

    let cap: Capability = ipc_recv().0;

    let fwd = cap.forward(c);
    ipc_send(&fwd, c);
}

app! { main }
