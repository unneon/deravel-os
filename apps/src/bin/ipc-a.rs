#![no_std]
#![no_main]

use deravel_kernel_api::*;

#[derive(Debug)]
struct Request {
    capability: Capability,
    type_: usize,
    text: [u8; 16],
    text_len: usize,
    data: [u8; 16],
    data_len: usize,
}

fn main() {
    let b = pid_by_name("ipc-b");

    let (fs_root_cap, fs) = ipc_recv::<Capability>();
    let req = Request {
        capability: fs_root_cap,
        type_: 0,
        text: *b"hello.txt\0\0\0\0\0\0\0",
        text_len: 9,
        data: [0; _],
        data_len: 0,
    };
    ipc_send(&req, fs);

    let cap = Capability::grant(b);
    ipc_send(&cap, b);
    yield_();

    let (req, req_sender) = ipc_recv::<Capability>();
    req.validate(req_sender);
}

app! { main }
