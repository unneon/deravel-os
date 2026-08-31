#![no_std]
#![no_main]
extern crate alloc;

include!(concat!(env!("OUT_DIR"), "/font.rs"));

use deravel_kernel_api::input::*;
use deravel_kernel_api::*;
use log::*;

struct Renderer {
    cursor_x: i32,
    cursor_y: i32,
    window_width: i32,
    window_height: i32,
    framebuffer: Framebuffer,
    window: Capability<Window>,
    events: &'static RingBuffer<InputEvent>,
}

impl Renderer {
    fn render_char(&mut self, c: u8) {
        if c == b' ' {
            self.cursor_x += FONT.width as i32;
        } else if c == b'\n' {
            self.cursor_x = FONT.leftpad as i32;
            self.cursor_y += FONT.height as i32;
        } else if c == b'\x08' {
            if self.cursor_x - (FONT.width as i32) < FONT.leftpad as i32 {
                if self.cursor_y - (FONT.height as i32) >= 0 {
                    self.cursor_y -= FONT.height as i32;
                    self.cursor_x = FONT.leftpad as i32;
                    while self.cursor_x + 2 * FONT.width as i32 <= self.window_width {
                        self.cursor_x += FONT.width as i32;
                    }
                } else {
                    return;
                }
            } else {
                self.cursor_x -= FONT.width as i32;
            }

            self.render_glyph(find_glyph(b'.').unwrap());
        } else if let Some(glyph) = find_glyph(c) {
            self.render_glyph(glyph);
            self.cursor_x += FONT.width as i32;
        }

        if self.cursor_x + FONT.width as i32 > self.window_width {
            self.cursor_x = FONT.leftpad as i32;
            self.cursor_y += FONT.height as i32;
        }
        if self.cursor_y + FONT.height as i32 > self.window_height {
            self.scroll_up();
            self.cursor_x = FONT.leftpad as i32;
            self.cursor_y -= FONT.height as i32;
        }
        self.window.draw();
    }

    fn render_glyph(&mut self, glyph: &Glyph) {
        for bitmap_y in 0..glyph.height as i32 {
            for bitmap_x in 0..glyph.width as i32 {
                let fb_x = self.cursor_x + bitmap_x + glyph.xmin;
                let fb_y = self.cursor_y + FONT.height as i32 - glyph.height as i32 + bitmap_y
                    - glyph.ymin;
                if fb_x >= 0 && fb_x < self.window_width && fb_y >= 0 && fb_y < self.window_height {
                    let color = glyph.bitmap[bitmap_y as usize * glyph.width + bitmap_x as usize];
                    self.framebuffer
                        .set_pixel(fb_x as usize, fb_y as usize, 0, color, 0, 255);
                }
            }
        }
    }

    fn scroll_up(&mut self) {
        let empty_start = self.window_height as usize - FONT.height;
        self.framebuffer.shift_rows(FONT.height, 0, empty_start);
        self.framebuffer
            .fill_rows(empty_start, self.window_height as usize, 0, 0, 0, 255);
    }

    fn clear_screen(&mut self) {
        self.framebuffer.fill(0, 0, 0, 255);
    }
}

impl ConsoleServer for Renderer {
    fn getchar(&mut self, _: &mut Ctx<Self>, _: ()) -> u8 {
        loop {
            let Some(event) = self.events.poll() else {
                yield_();
                continue;
            };

            // TODO: This assumes the entire sequence is always inserted at once.
            while let Some(following_event) = self.events.poll() {
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
                    KEY_DOT => b'.',
                    KEY_SPACE => b' ',
                    KEY_BACKSPACE => b'\x08',
                    _ => {
                        warn!("unrecognized {event:?}");
                        continue;
                    }
                };
            }
        }
    }

    fn putchar(&mut self, _: &mut Ctx<Self>, _: (), c: u8) {
        self.render_char(c);
    }
}

fn find_glyph(c: u8) -> Option<&'static Glyph> {
    FONT.glyphs.iter().find(|character| character.ascii == c)
}

fn main(args: TerminalArgs) {
    let width = 400;
    let height = 300;
    let window = args.windowing.create_window(width, height);
    let framebuffer = Framebuffer::map(width as usize, height as usize, window.framebuffer());
    let mut renderer = Renderer {
        cursor_x: FONT.leftpad as i32,
        cursor_y: 0,
        window_width: width as i32,
        window_height: height as i32,
        framebuffer,
        window,
        events: window.events(),
    };

    renderer.clear_screen();

    Dispatch::new(renderer).run();
}

app! { main }
