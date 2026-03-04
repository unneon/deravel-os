#![no_std]
#![no_main]

use deravel_kernel_api::deravel_types::ProcessId;
use deravel_kernel_api::deravel_types::drvli::filesystem;
use deravel_kernel_api::drvli::{ipc_bServer, ipc_serve_ipc_b};
use deravel_kernel_api::*;
use deravel_types::drvli::ipc_c;

struct Server {
    c: CallableCapability<ipc_c>,
}

impl ipc_bServer for Server {
    fn foo(&mut self, _: Capability, _: ProcessId, fs: CallableCapability<filesystem>) {
        let fs = forward_capability(fs, self.c.into());
        self.c.bar(fs);
    }
}

fn main(args: Args) {
    ipc_serve_ipc_b(Server { c: args.c });
}

app! { main ipc_b }
