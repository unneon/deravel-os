#![allow(clippy::collapsible_if)]
#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec::Vec;
use deravel_kernel_api::input::{EV_KEY, EV_REL, EV_SYN, KEY_ESC, KEY_LEFTALT, REL_X, REL_Y};
use deravel_kernel_api::*;
use log::*;

enum Shortcut {
    NotStarted,
    Alt,
}

struct Server {
    display: Capability<Display>,
    display_width: u32,
    display_height: u32,
    display_framebuffer: &'static mut [u8],
    windows: Vec<WindowData>,
    active_window: Option<usize>,
    cursor_x: u32,
    cursor_y: u32,
    global_shortcut: Shortcut,
}

struct WindowData {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    framebuffer: &'static mut [u8],
    memory: Capability<SharedMemory>,
    event_ring: Option<&'static RingBuffer<InputEvent>>,
}

#[derive(Clone, Copy)]
struct KeyboardTag;

#[derive(Clone, Copy)]
struct MouseTag;

impl WindowingServer for Server {
    fn create_window(&mut self, ctx: &mut Ctx<Self>, _: ()) -> Capability<Window> {
        let window_id = self.windows.len();
        let width = 400;
        let height = 300;
        let memory = allocate_shared_memory(width as usize * height as usize * 4);
        let framebuffer = unsafe { &mut *map_shared_memory(memory) };
        self.windows.push(WindowData {
            x: 0,
            y: 0,
            width,
            height,
            framebuffer,
            memory,
            event_ring: None,
        });
        self.active_window = Some(window_id);
        ctx.grant_capability(window_id)
    }
}

impl WindowServer<usize> for Server {
    fn width(&mut self, _: &mut Ctx<Self>, window_id: usize) -> u32 {
        self.windows[window_id].width
    }

    fn height(&mut self, _: &mut Ctx<Self>, window_id: usize) -> u32 {
        self.windows[window_id].height
    }

    fn framebuffer(&mut self, ctx: &mut Ctx<Self>, window_id: usize) -> Capability<SharedMemory> {
        ctx.forward_capability(self.windows[window_id].memory)
    }

    fn draw(&mut self, _: &mut Ctx<Self>, window_id: usize) {
        let window = &mut self.windows[window_id];
        for window_y in 0..window.height as usize {
            let display_y = window.y as usize + window_y;
            let display_offset =
                4 * display_y * self.display_width as usize + 4 * window.x as usize;
            let window_offset = 4 * window_y * window.width as usize;
            let size = 4 * window.width as usize;
            self.display_framebuffer[display_offset..][..size]
                .copy_from_slice(&window.framebuffer[window_offset..][..size]);
        }
        self.display.draw();
    }

    fn events(&mut self, window_id: usize) -> (Capability<SharedMemory>, usize) {
        let cap = allocate_shared_memory(PAGE_SIZE);
        let memory = map_shared_memory(cap);
        let ring = unsafe { RingBuffer::new_in_single_page(memory) };
        self.windows[window_id].event_ring = Some(ring);
        (cap, ring.untype().0.data.0.len())
    }
}

impl Observer<InputEvent, KeyboardTag> for Server {
    fn observe(&mut self, event: InputEvent, _: KeyboardTag) {
        if event.type_ == EV_KEY {
            match (&mut self.global_shortcut, event.code, event.value) {
                (Shortcut::NotStarted, KEY_LEFTALT, 1) => self.global_shortcut = Shortcut::Alt,
                (Shortcut::Alt, KEY_ESC, 1) => exit(),
                (Shortcut::Alt, KEY_LEFTALT, 0) => self.global_shortcut = Shortcut::NotStarted,
                (Shortcut::Alt, _, 1) => self.global_shortcut = Shortcut::NotStarted,
                _ => {}
            }
        }
        if let Some(window_id) = self.active_window {
            if let Some(event_ring) = self.windows[window_id].event_ring {
                event_ring.push(event);
            }
        }
    }
}

impl Observer<InputEvent, MouseTag> for Server {
    fn observe(&mut self, event: InputEvent, _: MouseTag) {
        if event.type_ == EV_REL {
            let delta = event.value as i32;
            if event.code == REL_X {
                self.cursor_x = self
                    .cursor_x
                    .saturating_add_signed(delta)
                    .min(self.display_width);
            } else if event.code == REL_Y {
                self.cursor_y = self
                    .cursor_y
                    .saturating_add_signed(delta)
                    .min(self.display_height);
            }
        } else if event.type_ == EV_SYN {
            self.display.update_cursor(self.cursor_x, self.cursor_y);
        }
    }
}

fn main(args: Args) {
    let width = args.display.width();
    let height = args.display.height();
    info!("found a {width}x{height} display");
    let framebuffer = unsafe { &mut *map_shared_memory(args.display.framebuffer()) };
    fill_screen(191, 215, 234, framebuffer, &args);
    initialize_cursor(255, 255, 255, 16, args.display);

    let server = Server {
        display_width: width,
        display_height: height,
        display_framebuffer: framebuffer,
        display: args.display,
        windows: Vec::new(),
        active_window: None,
        cursor_x: width / 2,
        cursor_y: height / 2,
        global_shortcut: Shortcut::NotStarted,
    };

    let mut dispatch = Dispatch::new(server);
    dispatch.observe(KeyboardTag, args.keyboard.events());
    dispatch.observe(MouseTag, args.mouse.events());
    loop {
        ipc_serve(&mut dispatch);
        yield_();
    }
}

fn fill_screen(red: u8, green: u8, blue: u8, framebuffer: &mut [u8], args: &Args) {
    for bgra in framebuffer.as_chunks_mut().0 {
        *bgra = [blue, green, red, 255];
    }
    args.display.draw();
}

fn initialize_cursor(red: u8, green: u8, blue: u8, size: usize, display: Capability<Display>) {
    let mut image = [0; 4 * 64 * 64];
    for y in 0..size.min(63) {
        for x in 0..size.min(63) {
            image.as_chunks_mut().0.chunks_mut(64).nth(y).unwrap()[x] = [red, green, blue, 255];
        }
    }
    display.set_cursor_image(&image);
}

app! { main Windowing }
