#![no_std]
#![no_main]
extern crate alloc;

include!(concat!(env!("OUT_DIR"), "/font.rs"));

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
    events: &'static RingBuffer<InputEvent>,
}

impl Renderer<'_> {
    fn render_char(&mut self, c: u8) {
        if c == b' ' {
            self.cursor_x += FONT.width as i32;
        } else if c == b'\n' {
            self.cursor_x = FONT.leftpad as i32;
            self.cursor_y += FONT.height as i32;
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

    fn scroll_up(&mut self) {
        let row_size = 4 * self.window_width as usize;
        let scroll_start = FONT.height * row_size;
        let clear_start = self.framebuffer.len() - scroll_start;
        self.framebuffer.copy_within(scroll_start.., 0);
        self.framebuffer[clear_start..]
            .as_chunks_mut()
            .0
            .fill([0, 0, 0, 255]);
    }

    fn clear_screen(&mut self) {
        self.framebuffer.as_chunks_mut().0.fill([0, 0, 0, 255]);
    }
}

impl ConsoleServer for Renderer<'_> {
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

fn main(args: Args) {
    let window = args.windowing.create_window();
    let framebuffer = unsafe { &mut *map_shared_memory(window.framebuffer()) };
    let mut renderer = Renderer {
        cursor_x: FONT.leftpad as i32,
        cursor_y: 0,
        window_width: window.width() as i32,
        window_height: window.height() as i32,
        framebuffer,
        window,
        events: window.events(),
    };

    renderer.clear_screen();

    let mut dispatch = Dispatch::new(renderer);
    loop {
        ipc_serve(&mut dispatch);
        yield_();
    }
}

app! { main Terminal }
