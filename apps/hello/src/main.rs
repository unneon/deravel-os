#![no_std]
#![no_main]

use deravel_kernel_api::putchar;

fn main() {
    for byte in "Hello, world!\n".bytes() {
        putchar(byte);
    }
}

deravel_kernel_api::app! { main }
