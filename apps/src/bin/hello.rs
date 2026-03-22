#![no_std]
#![no_main]

use deravel_kernel_api::*;

fn main(args: Args) {
    set_stdio(args.console);
    println!("Hello, world!");
}

app! { main Hello }
