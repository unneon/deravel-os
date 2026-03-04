#![no_std]
#![no_main]

use deravel_kernel_api::*;

struct Server {
    c: Capability<IpcC>,
}

impl IpcBServer for Server {
    fn foo(&mut self, _: Capability<IpcB>, _: ProcessId, fs: Capability<Filesystem>) {
        let fs = forward_capability(fs, self.c);
        self.c.bar(fs);
    }
}

fn main(args: Args) {
    ipc_serve_ipc_b(Server { c: args.c });
}

app! { main IpcB }
