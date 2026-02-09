#![no_std]
#![no_main]

use deravel_kernel_api::*;
use log::trace;

fn main() {
    let a = pid_by_name("ipc-a");

    let (cap, cap_sender) = ipc_recv::<Capability>();

    trace!("demonstrating {cap:?} to {cap_sender:?}");
    ipc_send(&cap, a);
}

app! { main }
