#![no_std]
#![no_main]
extern crate alloc;

use alloc::boxed::Box;
use deravel_kernel_api::*;

struct Server {
    c: Capability<IpcC>,
}

impl IpcBServer for Server {
    fn foo(&mut self, _: ProcessId, fs: Capability<Filesystem>) {
        let fs = forward_capability(fs, self.c);
        self.c.bar(fs);
    }
}

fn main(Args { c }: Args) {
    register_root_capability(Box::leak(Box::new(Server { c })));
    loop {
        ipc_serve();
        yield_();
    }
}

app! { main IpcB }
