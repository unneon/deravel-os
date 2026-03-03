#![no_std]
#![no_main]

use deravel_kernel_api::*;

fn main(caps: Capabilities) {
    println!("Hello, world!");
}

app! { main hello_prelude }
