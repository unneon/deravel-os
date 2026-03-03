#![no_std]
#![no_main]
extern crate alloc;

use deravel_kernel_api::deravel_types::capability::CallableCapability;
use deravel_kernel_api::deravel_types::interfaces::filesystem;
use deravel_kernel_api::*;
use log::{debug, trace};

fn main(caps: Capabilities) {
    let (cap, _) = cap_recv::<CallableCapability<filesystem>>();
    let data = cap.read("secret.txt");
    let text = core::str::from_utf8(&data).unwrap();
    debug!("read {text:?} from file");
}

app! { main ipc_c_prelude }
