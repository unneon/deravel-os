#![no_std]
#![no_main]

use deravel_kernel_api::*;

fn main(args: Args) {
    let window = args.windowing.create_window();
    let framebuffer = window.framebuffer();
    let (framebuffer_ptr, framebuffer_len) = unsafe { syscall::map_shared_memory(framebuffer) };
    let framebuffer = unsafe { core::slice::from_raw_parts_mut(framebuffer_ptr, framebuffer_len) };

    for [b, g, r, a] in framebuffer.as_chunks_mut().0 {
        *b = 5;
        *g = 22;
        *r = 37;
        *a = 255;
    }

    window.draw();

    loop {
        yield_();
    }
}

app! { main Terminal }
