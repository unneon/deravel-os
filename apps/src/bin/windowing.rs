#![allow(clippy::collapsible_if)]
#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec::Vec;
use deravel_kernel_api::input::{
    ABS_X, ABS_Y, BTN_LEFT, EV_ABS, EV_KEY, EV_REL, EV_SYN, KEY_ESC, KEY_LEFTALT, KEY_Q, KEY_T,
    REL_X, REL_Y,
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
    display_width: u32,
    display_height: u32,
    display_framebuffer: Framebuffer,
    windows: Vec<WindowData>,
    active_window: Option<usize>,
    cursor_x: i32,
    cursor_y: i32,
    fs: Capability<Filesystem>,
    image_viewer: Capability<ImageViewerSpawner>,
    net: Capability<Network>,
    shutdown: Capability<Shutdown>,
    global_shortcut: Shortcut,
    shell_spawner: Capability<ShellSpawner>,
    terminal_spawner: Capability<TerminalSpawner>,
    abs_x_info: InputAbsinfo,
    abs_y_info: InputAbsinfo,
}

struct WindowData {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
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

impl Server {
    fn draw_window(&mut self, window_id: usize) {
        let window = &self.windows[window_id];
        self.display_framebuffer.copy_from_rect(
            window.x as isize,
            window.y as isize,
            &window.framebuffer,
        );
    }
}

impl WindowingServer for Server {
    fn create_window(
        &mut self,
        ctx: &mut Ctx<Self>,
        _: (),
        width: u32,
        height: u32,
    ) -> Capability<Window> {
        let window_id = self.windows.len();
        let (framebuffer, memory) = Framebuffer::alloc(width as usize, height as usize);
        self.windows.push(WindowData {
            x: self.cursor_x - width as i32 / 2,
            y: self.cursor_y - height as i32 / 2,
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
    fn framebuffer(&mut self, ctx: &mut Ctx<Self>, window_id: usize) -> Capability<SharedMemory> {
        ctx.forward_to_sender(self.windows[window_id].memory)
    }

    fn draw(&mut self, _: &mut Ctx<Self>, window_id: usize) {
        self.draw_window(window_id);
        if let Some(active) = self.active_window
            && active != window_id
        {
            self.draw_window(active);
        }
        self.display.draw();
    }

    fn events(
        &mut self,
        ctx: &mut Ctx<Self>,
        window_id: usize,
    ) -> (Capability<SharedMemory>, usize) {
        let (memory, cap) = alloc_shared(PAGE_SIZE);
        let ring = unsafe { RingBuffer::new_in_single_page(memory) };
        self.windows[window_id].event_ring = Some(ring);
        (ctx.forward_to_sender(cap), ring.untype().0.data.0.len())
    }
}

impl Observer<InputEvent, KeyboardTag> for Server {
    fn observe(&mut self, mut ctx: OCtx<Self>, event: InputEvent, _: KeyboardTag) {
        if event.type_ == EV_KEY {
            match (self.global_shortcut, event.code, event.value) {
                (Shortcut::NotStarted, KEY_LEFTALT, 1) => self.global_shortcut = Shortcut::Alt,
                (Shortcut::Alt, KEY_ESC, 1) => self.shutdown.shutdown(),
                (Shortcut::Alt, KEY_T, 1) => {
                    let term = self.terminal_spawner.spawn(ctx.grant_to_kernel(()));
                    let term = forward(term, Actor::Kernel);
                    let fs = forward(self.fs, Actor::Kernel);
                    let image_viewer = forward(self.image_viewer, Actor::Kernel);
                    let windowing = ctx.grant_to_kernel(());
                    let net = forward(self.net, Actor::Kernel);
                    let shutdown = forward(self.shutdown, Actor::Kernel);
                    self.shell_spawner
                        .spawn(term, fs, image_viewer, windowing, net, shutdown);
                    self.active_window = None;
                    self.global_shortcut = Shortcut::NotStarted;
                }
                (Shortcut::Alt, KEY_Q, 1) => {
                    if let Some(window_id) = self.active_window.take() {
                        let window = &mut self.windows[window_id];
                        window.status = WindowStatus::Closed;
                        self.display_framebuffer.fill_rect(
                            window.x.max(0).min(self.display_width as i32) as usize,
                            window.y.max(0).min(self.display_height as i32) as usize,
                            (window.x + window.width as i32)
                                .max(0)
                                .min(self.display_width as i32)
                                as usize,
                            (window.y + window.height as i32)
                                .max(0)
                                .min(self.display_height as i32)
                                as usize,
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
                        && self.cursor_x < window.x + window.width as i32
                        && self.cursor_y >= window.y
                        && self.cursor_y < window.y + window.height as i32
                        && window.status == WindowStatus::Open
                    {
                        self.active_window = Some(window_index);
                    }
                }
            }
        } else if event.type_ == EV_REL {
            let delta = event.value as i32;
            if event.code == REL_X {
                self.cursor_x = (self.cursor_x + delta)
                    .max(0)
                    .min(self.display_width as i32);
            } else if event.code == REL_Y {
                self.cursor_y = (self.cursor_y + delta)
                    .max(0)
                    .min(self.display_height as i32);
            }
        } else if event.type_ == EV_ABS {
            if event.code == ABS_X {
                self.cursor_x = from_abs(event.value, &self.abs_x_info, self.display_width);
            } else if event.code == ABS_Y {
                self.cursor_y = from_abs(event.value, &self.abs_y_info, self.display_height);
            }
        } else if event.type_ == EV_SYN {
            self.display
                .update_cursor(self.cursor_x as u32, self.cursor_y as u32);
        }
    }
}

fn main(args: WindowingArgs) {
    let width = args.display.width();
    let height = args.display.height();
    info!("found a {width}x{height} display");

    let mut framebuffer =
        Framebuffer::map(width as usize, height as usize, args.display.framebuffer());
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
        cursor_x: width as i32 / 2,
        cursor_y: height as i32 / 2,
        fs: args.fs,
        image_viewer: args.image_viewer,
        net: args.net,
        shutdown: args.shutdown,
        global_shortcut: Shortcut::NotStarted,
        shell_spawner: args.shell,
        terminal_spawner: args.terminal,
        abs_x_info: args.mouse.absinfo(ABS_X),
        abs_y_info: args.mouse.absinfo(ABS_Y),
    };

    let mut dispatch = Dispatch::new(server);
    dispatch.observe(KeyboardTag, args.keyboard.events());
    dispatch.observe(MouseTag, args.mouse.events());
    dispatch.run();
}

fn initialize_cursor(red: u8, green: u8, blue: u8, size: usize, display: Capability<Display>) {
    let mut image = Framebuffer::map(64, 64, display.cursor_image_buffer());
    image.fill_rect(0, 0, size, size, red, green, blue, 255);
    display.cursor_image_modified()
}

fn from_abs(value: u32, info: &InputAbsinfo, res: u32) -> i32 {
    (((value - info.min) as u64 * res as u64) / (info.max - info.min) as u64) as i32
}

app! { main }
