#![no_std]
#![no_main]

use deravel_kernel_api::*;

fn main(_: Args) {
    println!("Hello, world!");
}

app! { main Hello }
