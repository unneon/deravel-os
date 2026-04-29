#![no_std]
#![no_main]

use deravel_kernel_api::*;
use log::debug;

fn main(args: Args) {
    let width = args.display.width() as usize;
    let height = args.display.height() as usize;
    debug!("found a {width}x{height} display");
    let framebuffer = args.display.framebuffer();
    let (framebuffer, framebuffer_len) = unsafe { syscall::map_shared_memory(framebuffer) };
    let framebuffer = unsafe { core::slice::from_raw_parts_mut(framebuffer, framebuffer_len) };
    loop {
        for bgra in framebuffer.as_chunks_mut().0 {
            *bgra = [0, 0, 255, 255];
        }
        args.display.draw();
        for _ in 0..1_000_000_000 {
            unsafe { core::arch::asm!("nop") }
        }
        for bgra in framebuffer.as_chunks_mut().0 {
            *bgra = [255, 0, 0, 255];
        }
        args.display.draw();
        for _ in 0..1_000_000_000 {
            unsafe { core::arch::asm!("nop") }
        }
    }
}

app! { main Windowing }
