#![feature(iter_array_chunks)]
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use deravel_kernel_api::*;
use log::debug;

fn main(args: Args) {
    let width = args.display.width() as usize;
    let height = args.display.height() as usize;
    debug!("found display {width}x{height}");
    let mut framebuffer = vec![0u8; 4 * width * height];
    debug!("framebuffer allocated");
    loop {
        for bgra in framebuffer.as_chunks_mut().0 {
            *bgra = [0, 0, 255, 255];
        }
        args.display.draw(&framebuffer);
        for bgra in framebuffer.as_chunks_mut().0 {
            *bgra = [255, 0, 0, 255];
        }
        args.display.draw(&framebuffer);
    }
}

app! { main Windowing }
