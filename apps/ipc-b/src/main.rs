#![no_std]
#![no_main]

use deravel_kernel_api::{Capability, app, pid, println};

fn main() {
    println!("B launched with pid {}", pid());
    let cap = Capability::guess(0x2000000);
    println!("B received {cap:?} from A (todo)");
    let forwarded = cap.forward(2);
    println!("B forwarded {cap:?} as {forwarded:?} for C")
}

app! { main }
