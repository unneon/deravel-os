#![no_std]
#![no_main]
extern crate alloc;

use deravel_kernel_api::*;
use log::debug;

struct Server;

impl IpcCServer for Server {
    fn bar(&mut self, _: &mut Ctx<Self>, _: (), fs: Capability<Filesystem>) {
        let data = fs.read("secret.txt");
        let text = str::from_utf8(&data).unwrap();
        debug!("read {text:?} from file");
    }
}

fn main(_: Args) {
    let mut dispatch = Dispatch::new(Server);
    loop {
        ipc_serve(&mut dispatch);
        yield_();
    }
}

app! { main IpcC }
