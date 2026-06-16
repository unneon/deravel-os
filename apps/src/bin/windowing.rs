#![allow(clippy::collapsible_if)]
#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec::Vec;
use deravel_kernel_api::input::{
    BTN_LEFT, EV_KEY, EV_REL, EV_SYN, KEY_ESC, KEY_LEFTALT, KEY_Q, KEY_T, REL_X, REL_Y,
};
use deravel_kernel_api::*;
use log::*;

#[derive(Clone, Copy)]
enum Shortcut {
    NotStarted,
    Alt,
}

struct Server {
    display: Capability<Display>,
    display_width: usize,
    display_height: usize,
    display_framebuffer: Framebuffer,
    windows: Vec<WindowData>,
    active_window: Option<usize>,
    cursor_x: usize,
    cursor_y: usize,
    fs: Capability<Filesystem>,
    net: Capability<Network>,
    shutdown: Capability<Shutdown>,
    global_shortcut: Shortcut,
    shell_spawner: Capability<ShellSpawner>,
    terminal_spawner: Capability<TerminalSpawner>,
}

struct WindowData {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    status: WindowStatus,
    framebuffer: Framebuffer,
    memory: Capability<SharedMemory>,
    event_ring: Option<&'static RingBuffer<InputEvent>>,
}

#[derive(Eq, PartialEq)]
enum WindowStatus {
    Open,
    Closed,
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
        let (framebuffer, memory) = Framebuffer::alloc(width, height);
        self.windows.push(WindowData {
            x: self.cursor_x - width / 2,
            y: self.cursor_y - height / 2,
            width,
            height,
            status: WindowStatus::Open,
            framebuffer,
            memory,
            event_ring: None,
        });
        self.active_window = Some(window_id);
        ctx.grant_to_sender(window_id)
    }
}

impl WindowServer<usize> for Server {
    fn width(&mut self, _: &mut Ctx<Self>, window_id: usize) -> usize {
        self.windows[window_id].width
    }

    fn height(&mut self, _: &mut Ctx<Self>, window_id: usize) -> usize {
        self.windows[window_id].height
    }

    fn framebuffer(&mut self, ctx: &mut Ctx<Self>, window_id: usize) -> Capability<SharedMemory> {
        ctx.forward_to_sender(self.windows[window_id].memory)
    }

    fn draw(&mut self, _: &mut Ctx<Self>, window_id: usize) {
        let window = &mut self.windows[window_id];
        for window_y in 0..window.height {
            let display_y = window.y + window_y;
            self.display_framebuffer.row(display_y)[window.x..][..window.width]
                .copy_from_slice(window.framebuffer.row(window_y))
        }
        self.display.draw();
    }

    fn events(&mut self, window_id: usize) -> (Capability<SharedMemory>, usize) {
        let (memory, cap) = alloc_shared(PAGE_SIZE);
        let ring = unsafe { RingBuffer::new_in_single_page(memory) };
        self.windows[window_id].event_ring = Some(ring);
        (cap, ring.untype().0.data.0.len())
    }
}

impl Observer<InputEvent, KeyboardTag> for Server {
    fn observe(&mut self, mut ctx: OCtx<Self>, event: InputEvent, _: KeyboardTag) {
        if event.type_ == EV_KEY {
            match (self.global_shortcut, event.code, event.value) {
                (Shortcut::NotStarted, KEY_LEFTALT, 1) => self.global_shortcut = Shortcut::Alt,
                (Shortcut::Alt, KEY_ESC, 1) => exit(),
                (Shortcut::Alt, KEY_T, 1) => {
                    let term = self.terminal_spawner.spawn(ctx.grant_to_kernel(()));
                    let term = forward(term, Actor::Kernel);
                    let fs = forward(self.fs, Actor::Kernel);
                    let net = forward(self.net, Actor::Kernel);
                    let shutdown = forward(self.shutdown, Actor::Kernel);
                    self.shell_spawner.spawn(term, fs, net, shutdown);
                    self.active_window = None;
                    self.global_shortcut = Shortcut::NotStarted;
                }
                (Shortcut::Alt, KEY_Q, 1) => {
                    if let Some(window_id) = self.active_window.take() {
                        let window = &mut self.windows[window_id];
                        window.status = WindowStatus::Closed;
                        self.display_framebuffer.fill_rect(
                            window.x,
                            window.y,
                            window.x + window.width,
                            window.y + window.height,
                            191,
                            215,
                            234,
                            255,
                        );
                        self.display.draw();
                    }
                }
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
    fn observe(&mut self, _: OCtx<Self>, event: InputEvent, _: MouseTag) {
        if event.type_ == EV_KEY {
            if event.code == BTN_LEFT && event.value == 1 {
                for (window_index, window) in self.windows.iter().enumerate() {
                    if self.cursor_x >= window.x
                        && self.cursor_x < window.x + window.width
                        && self.cursor_y >= window.y
                        && self.cursor_y < window.y + window.height
                        && window.status == WindowStatus::Open
                    {
                        self.active_window = Some(window_index);
                    }
                }
            }
        } else if event.type_ == EV_REL {
            let delta = event.value as i32 as isize;
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
            self.display
                .update_cursor(self.cursor_x as u32, self.cursor_y as u32);
        }
    }
}

fn main(args: Args) {
    let width = args.display.width() as usize;
    let height = args.display.height() as usize;
    info!("found a {width}x{height} display");

    let mut framebuffer = Framebuffer::map(width, height, args.display.framebuffer());
    framebuffer.fill(191, 215, 234, 255);
    args.display.draw();

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
        fs: args.fs,
        net: args.net,
        shutdown: args.shutdown,
        global_shortcut: Shortcut::NotStarted,
        shell_spawner: args.shell,
        terminal_spawner: args.terminal,
    };

    let mut dispatch = Dispatch::new(server);
    dispatch.observe(KeyboardTag, args.keyboard.events());
    dispatch.observe(MouseTag, args.mouse.events());
    dispatch.run();
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
