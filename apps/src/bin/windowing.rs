#![no_std]
#![no_main]
extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::RefCell;
use deravel_kernel_api::*;
use log::debug;

struct Server {
    display: Capability<Display>,
    display_width: u32,
    display_framebuffer: &'static mut [u8],
    windows: Vec<WindowData>,
    event_receivers: &'static RefCell<Vec<&'static RingBuffer<InputEvent>>>,
}

struct WindowData {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    framebuffer: &'static mut [u8],
    framebuffer_memory: Capability<SharedMemory>,
    event_ring: Option<&'static RingBuffer<InputEvent>>,
}

impl WindowingServer for Server {
    fn create_window(&mut self, ctx: &mut Ctx<Self>, _: ()) -> Capability<Window> {
        let window_id = self.windows.len();
        let width = 400;
        let height = 300;
        let window_framebuffer_cap =
            unsafe { syscall::allocate_shared_memory(width as usize * height as usize * 4) };
        let (framebuffer_ptr, framebuffer_len) =
            unsafe { syscall::map_shared_memory(window_framebuffer_cap) };
        let window_framebuffer_data =
            unsafe { core::slice::from_raw_parts_mut(framebuffer_ptr, framebuffer_len) };
        self.windows.push(WindowData {
            x: 0,
            y: 0,
            width,
            height,
            framebuffer: window_framebuffer_data,
            framebuffer_memory: window_framebuffer_cap,
            event_ring: None,
        });
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
        ctx.forward_capability(self.windows[window_id].framebuffer_memory)
    }

    fn draw(&mut self, _: &mut Ctx<Self>, window_id: usize) {
        let window = &mut self.windows[window_id];
        for window_y in 0..window.height as usize {
            let display_y = window.y as usize + window_y;
            let display_offset =
                4 * display_y * self.display_width as usize + 4 * window.x as usize;
            let window_offset = 4 * window_y * window.width as usize;
            let size = 4 * window.width as usize;
            self.display_framebuffer[display_offset..display_offset + size]
                .copy_from_slice(&window.framebuffer[window_offset..window_offset + size]);
        }
        self.display.draw();
    }

    fn events(&mut self, window_id: usize) -> (Capability<SharedMemory>, usize) {
        let cap = unsafe { syscall::allocate_shared_memory(PAGE_SIZE) };
        let memory = unsafe { syscall::map_shared_memory(cap).0 };
        let ring = unsafe { RingBuffer::new_in_single_page(memory) };
        self.windows[window_id].event_ring = Some(ring);
        self.event_receivers.borrow_mut().push(ring);
        (cap, ring.untype().0.data.0.len())
    }
}

fn main(args: Args) {
    let width = args.display.width();
    let height = args.display.height();
    debug!("found a {width}x{height} display");
    let framebuffer = args.display.framebuffer();
    let (framebuffer, framebuffer_len) = unsafe { syscall::map_shared_memory(framebuffer) };
    let framebuffer = unsafe { core::slice::from_raw_parts_mut(framebuffer, framebuffer_len) };
    fill_screen(191, 215, 234, framebuffer, &args);

    let event_receivers = Box::leak(Box::new(RefCell::new(Vec::new())));
    let server = Server {
        display_width: width,
        display_framebuffer: framebuffer,
        display: args.display,
        windows: Vec::new(),
        event_receivers,
    };

    let keyboard = args.keyboard.events();
    let mut dispatch = Dispatch::new(server);
    loop {
        ipc_serve(&mut dispatch);

        while let Some(event) = keyboard.poll() {
            for event_receiver in event_receivers.borrow().as_slice() {
                event_receiver.push(event);
            }
        }

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
