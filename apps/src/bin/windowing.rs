#![feature(iter_array_chunks)]
#![no_std]
#![no_main]

use deravel_kernel_api::*;
use log::debug;

fn main(args: Args) {
    let width = args.display.width() as usize;
    let height = args.display.height() as usize;
    debug!("found display {width}x{height}");
    let framebuffer = args.display.framebuffer();
    let (framebuffer, framebuffer_len) = unsafe { syscall::map_shared_memory(framebuffer) };
    let framebuffer = unsafe { core::slice::from_raw_parts_mut(framebuffer, framebuffer_len) };
    assert_eq!(framebuffer_len, 4 * width * height);
    debug!("framebuffer mapped");
    loop {
        for bgra in framebuffer.as_chunks_mut().0 {
            *bgra = [0, 0, 255, 255];
        }
        args.display.draw();
        for bgra in framebuffer.as_chunks_mut().0 {
            *bgra = [255, 0, 0, 255];
        }
        args.display.draw();
    }
}

app! { main Windowing }
