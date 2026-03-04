#![no_std]
#![no_main]

use deravel_kernel_api::drvli::{IpcBServer, IpcCClient, ipc_serve_ipc_b};
use deravel_kernel_api::*;
use deravel_types::ProcessId;
use deravel_types::capability::Capability;
use deravel_types::drvli::Filesystem;
use deravel_types::drvli::IpcC;

struct Server {
    c: Capability<IpcC>,
}

impl IpcBServer for Server {
    fn foo(&mut self, _: RawCapability, _: ProcessId, fs: Capability<Filesystem>) {
        let fs = forward_capability(fs, self.c);
        self.c.bar(fs);
    }
}

fn main(args: Args) {
    ipc_serve_ipc_b(Server { c: args.c });
}

app! { main IpcB }
