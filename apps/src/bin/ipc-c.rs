#![no_std]
#![no_main]

use deravel_kernel_api::drvli::{FilesystemClient, IpcCServer, ipc_serve_ipc_c};
use deravel_kernel_api::*;
use deravel_types::ProcessId;
use deravel_types::capability::Capability;
use deravel_types::drvli::Filesystem;
use log::debug;

struct Server;

impl IpcCServer for Server {
    fn bar(&mut self, _: RawCapability, _: ProcessId, fs: Capability<Filesystem>) {
        let data = fs.read("secret.txt");
        let text = str::from_utf8(&data).unwrap();
        debug!("read {text:?} from file");
    }
}

fn main(_: Args) {
    ipc_serve_ipc_c(Server);
}

app! { main IpcC }
