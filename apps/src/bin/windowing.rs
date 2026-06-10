#![allow(clippy::collapsible_if)]
#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec::Vec;
use deravel_kernel_api::*;
use log::debug;

struct Server {
    display: Capability<Display>,
    display_width: u32,
    display_framebuffer: &'static mut [u8],
    windows: Vec<WindowData>,
    active_window: Option<usize>,
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

impl Observer<InputEvent, ()> for Server {
    fn observe(&mut self, event: InputEvent, _: ()) {
        if let Some(window_id) = self.active_window {
            if let Some(event_ring) = self.windows[window_id].event_ring {
                event_ring.push(event);
            }
        }
    }
}

fn main(args: Args) {
    let width = args.display.width();
    let height = args.display.height();
    debug!("found a {width}x{height} display");
    let framebuffer = unsafe { &mut *map_shared_memory(args.display.framebuffer()) };
    fill_screen(191, 215, 234, framebuffer, &args);

    let server = Server {
        display_width: width,
        display_framebuffer: framebuffer,
        display: args.display,
        windows: Vec::new(),
        active_window: None,
    };

    let mut dispatch = Dispatch::new(server);
    dispatch.observe((), args.keyboard.events());
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

app! { main Windowing }
