#![no_std]
#![no_main]

use deravel_kernel_api::*;

fn main() {
    let b = pid_by_name("ipc-b");

    let cap = Capability::grant(b);
    ipc_send(&cap, b);
    yield_();

    let (req, req_sender) = ipc_recv::<Capability>();
    req.validate(req_sender);
}

app! { main }
