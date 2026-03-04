#![no_std]
#![no_main]

use deravel_kernel_api::CallableCapability;
use deravel_kernel_api::deravel_types::ProcessId;
use deravel_kernel_api::drvli::{ipc_cServer, ipc_serve_ipc_c};
use deravel_kernel_api::*;
use deravel_types::drvli::filesystem;
use log::debug;

struct Server;

impl ipc_cServer for Server {
    fn bar(&mut self, _: Capability, _: ProcessId, fs: CallableCapability<filesystem>) {
        let data = fs.read("secret.txt");
        let text = str::from_utf8(&data).unwrap();
        debug!("read {text:?} from file");
    }
}

fn main(_: Args) {
    ipc_serve_ipc_c(Server);
}

app! { main ipc_c }
