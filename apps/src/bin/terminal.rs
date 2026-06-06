#![no_std]
#![no_main]
extern crate alloc;

include!(concat!(env!("OUT_DIR"), "/font.rs"));

use alloc::boxed::Box;
use deravel_kernel_api::input::*;
use deravel_kernel_api::*;
use log::warn;

struct Renderer<'a> {
    cursor_x: i32,
    cursor_y: i32,
    window_width: i32,
    window_height: i32,
    framebuffer: &'a mut [u8],
    window: Capability<Window>,
    last_polled_event: f64,
}

impl Renderer<'_> {
    fn render_char(&mut self, c: u8) {
        if c == b' ' {
            self.cursor_x += FONT.width as i32;
            return;
        } else if c == b'\n' {
            self.cursor_x = 0;
            self.cursor_y += FONT.height as i32;
            return;
        } else if let Some(glyph) = FONT
            .characters
            .iter()
            .find(|character| character.ascii == c)
        {
            for bitmap_y in 0..glyph.height as i32 {
                for bitmap_x in 0..glyph.width as i32 {
                    let fb_x = self.cursor_x + bitmap_x + glyph.xmin;
                    let fb_y = self.cursor_y + FONT.height as i32 - glyph.height as i32 + bitmap_y
                        - glyph.ymin;
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
            self.cursor_x += FONT.width as i32;
        }

        if self.cursor_x + FONT.width as i32 > self.window_width {
            self.cursor_x = 0;
            self.cursor_y += FONT.height as i32;
        }
        if self.cursor_y + FONT.height as i32 > self.window_height {
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

impl ConsoleServer for Renderer<'_> {
    fn getchar(&mut self, _: ProcessId) -> u8 {
        loop {
            let event = self.window.poll_event();
            if event.type_ == 0 {
                loop {
                    let time = system_time();
                    if time > self.last_polled_event + 0.1 {
                        self.last_polled_event = time;
                        break;
                    }
                    yield_();
                }
                continue;
            };

            loop {
                let following_event = self.window.poll_event();
                if following_event.type_ == 0 {
                    break;
                }
            }
            if event.value == 1 {
                break match event.code {
                    KEY_A => b'a',
                    KEY_B => b'b',
                    KEY_C => b'c',
                    KEY_D => b'd',
                    KEY_E => b'e',
                    KEY_F => b'f',
                    KEY_G => b'g',
                    KEY_H => b'h',
                    KEY_I => b'i',
                    KEY_J => b'j',
                    KEY_K => b'k',
                    KEY_L => b'l',
                    KEY_M => b'm',
                    KEY_N => b'n',
                    KEY_O => b'o',
                    KEY_P => b'p',
                    KEY_Q => b'q',
                    KEY_R => b'r',
                    KEY_S => b's',
                    KEY_T => b't',
                    KEY_U => b'u',
                    KEY_V => b'v',
                    KEY_W => b'w',
                    KEY_X => b'x',
                    KEY_Y => b'y',
                    KEY_Z => b'z',
                    KEY_ENTER => b'\r',
                    _ => {
                        warn!("unrecognized {event:?}");
                        continue;
                    }
                };
            }
        }
    }

    fn putchar(&mut self, _: ProcessId, c: u8) {
        self.render_char(c);
    }
}

unsafe impl Send for Renderer<'_> {}
unsafe impl Sync for Renderer<'_> {}

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
        last_polled_event: f64::NEG_INFINITY,
    };

    renderer.clear_screen();
    register_root_capability(Box::leak(Box::new(renderer)));
    loop {
        ipc_serve();
        yield_();
    }
}

app! { main Terminal }
