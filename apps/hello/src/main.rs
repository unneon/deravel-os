#![no_std]
#![no_main]

use deravel_kernel_api::println;

fn main() {
    println!("Hello, world!");
}

deravel_kernel_api::app! { main }
