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
    let mut keyboard = args.keyboard.events();
    loop {
        while let Some(event) = keyboard.next() {
            if event.type_ != 0 {
                debug!("{event:?}");
            }
        }
        fill_screen(255, 0, 0, framebuffer, &args);
        fill_screen(0, 0, 255, framebuffer, &args);
    }
}

fn fill_screen(red: u8, green: u8, blue: u8, framebuffer: &mut [u8], args: &Args) {
    for bgra in framebuffer.as_chunks_mut().0 {
        *bgra = [blue, green, red, 255];
    }
    args.display.draw();
}

app! { main Windowing }
