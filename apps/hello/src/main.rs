#![no_std]
#![no_main]

use deravel_kernel_api::{app, println};

fn main() {
    println!("Hello, world!");
}

app! { main }
