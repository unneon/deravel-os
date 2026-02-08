#![no_std]
#![no_main]

use deravel_kernel_api::{Capability, app, pid, println};

fn main() {
    println!("C launched with pid {}", pid());
    let cap = Capability::guess(0x2001000);
    println!("C received {cap:?} from B (todo)");
    println!("C requested sth from A with {cap:?} (todo)");
}

app! { main }
