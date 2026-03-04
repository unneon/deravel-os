#![no_std]
#![no_main]

use deravel_kernel_api::CallableCapability;
use deravel_kernel_api::*;
use deravel_types::drvli::filesystem;
use log::{debug, trace};

fn main(_: Args) {
    let (cap, _) = cap_recv::<CallableCapability<filesystem>>();
    let data = cap.read("secret.txt");
    let text = core::str::from_utf8(&data).unwrap();
    debug!("read {text:?} from file");
}

app! { main ipc_c }
