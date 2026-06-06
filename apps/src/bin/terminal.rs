#![no_std]
#![no_main]
extern crate alloc;

include!(concat!(env!("OUT_DIR"), "/font.rs"));

use deravel_kernel_api::*;
use log::debug;
use rand::{RngExt, SeedableRng};

struct Renderer<'a> {
    cursor_x: i32,
    cursor_y: i32,
    window_width: i32,
    window_height: i32,
    framebuffer: &'a mut [u8],
    window: Capability<Window>,
}

impl Renderer<'_> {
    fn render_char(&mut self, c: u8) {
        if c == b' ' {
            self.cursor_x += 10;
            return;
        } else if c == b'\n' {
            self.cursor_x = 0;
            self.cursor_y += 17;
            return;
        } else if let Some(glyph) = FONT.iter().find(|glyph| glyph.ascii == c) {
            for bitmap_y in 0..glyph.height as i32 {
                for bitmap_x in 0..glyph.width as i32 {
                    let fb_x = self.cursor_x + bitmap_x + glyph.xmin;
                    let fb_y = self.cursor_y + 17 - glyph.height as i32 + bitmap_y - glyph.ymin;
                    if fb_x >= 0
                        && fb_x < self.window_width
                        && fb_y >= 0
                        && fb_y < self.window_height
                    {
                        let [b, g, r, _] = &mut self.framebuffer.as_chunks_mut().0
                            [fb_y as usize * self.window_width as usize + fb_x as usize];
                        let color =
                            glyph.bitmap[bitmap_y as usize * glyph.width + bitmap_x as usize];
                        *b = 0;
                        *g = color;
                        *r = 0;
                    }
                }
            }
            self.cursor_x += glyph.width as i32;
        }

        if self.cursor_x + 10 > self.window_width {
            self.cursor_x = 0;
            self.cursor_y += 17;
        }
        if self.cursor_y + 17 > self.window_height {
            self.cursor_x = 0;
            self.cursor_y = 0;
            self.clear_screen();
        }
        self.window.draw();
    }

    fn clear_screen(&mut self) {
        for [b, g, r, a] in self.framebuffer.as_chunks_mut().0 {
            *b = 0;
            *g = 0;
            *r = 0;
            *a = 255;
        }
    }
}

fn main(args: Args) {
    let window = args.windowing.create_window();
    let framebuffer = window.framebuffer();
    let (framebuffer_ptr, framebuffer_len) = unsafe { syscall::map_shared_memory(framebuffer) };
    let framebuffer = unsafe { core::slice::from_raw_parts_mut(framebuffer_ptr, framebuffer_len) };
    let mut renderer = Renderer {
        cursor_x: 0,
        cursor_y: 0,
        window_width: window.width() as i32,
        window_height: window.height() as i32,
        framebuffer,
        window,
    };

    renderer.clear_screen();
    for &c in "Hello, world!\nuwu".as_bytes() {
        renderer.render_char(c);
    }

    let start_time = riscv::register::time::read();
    let timebase_frequency = unsafe { syscall::riscv_timebase_frequency() } as f64;
    let mut last_time = 0.;
    let mut rng = rand::rngs::SmallRng::seed_from_u64(907);
    loop {
        let time = (riscv::register::time::read() - start_time) as f64 / timebase_frequency;
        if time - last_time > 0.1 {
            renderer.render_char(rng.random_range(b'a'..=b'z'));
            last_time += 0.1;
            let ev = window.poll_event();
            if ev.type_ != 0 {
                debug!("poll event: {ev:?}");
            }
        }

        yield_();
    }
}

app! { main Terminal }
