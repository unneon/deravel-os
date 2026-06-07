#![no_std]
#![no_main]
extern crate alloc;

use deravel_kernel_api::*;

struct Server {
    c: Capability<IpcC>,
}

impl IpcBServer for Server {
    fn foo(&mut self, _: &mut Ctx<Self>, _: (), fs: Capability<Filesystem>) {
        let fs = forward_capability_by_cap(fs, self.c);
        self.c.bar(fs);
    }
}

fn main(Args { c }: Args) {
    let mut dispatch = Dispatch::new(Server { c });
    loop {
        ipc_serve(&mut dispatch);
        yield_();
    }
}

app! { main IpcB }
