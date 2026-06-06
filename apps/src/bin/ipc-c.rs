#![no_std]
#![no_main]
extern crate alloc;

use alloc::boxed::Box;
use deravel_kernel_api::*;
use log::debug;

struct Server;

impl IpcCServer for Server {
    fn bar(&mut self, _: ProcessId, fs: Capability<Filesystem>) {
        let data = fs.read("secret.txt");
        let text = str::from_utf8(&data).unwrap();
        debug!("read {text:?} from file");
    }
}

fn main(_: Args) {
    register_root_capability(Box::leak(Box::new(Server)));
    loop {
        ipc_serve();
        yield_();
    }
}

app! { main IpcC }
