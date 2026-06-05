#![no_std]
#![no_main]
extern crate alloc;

use alloc::boxed::Box;
use core::cell::RefCell;
use deravel_kernel_api::*;
use log::debug;

struct Server {
    width: u32,
    height: u32,
    framebuffer: &'static RefCell<&'static mut [u8]>,
    cap: Capability<Display>,
}

struct WindowData {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    window_framebuffer_data: &'static mut [u8],
    window_framebuffer_cap: Capability<SharedMemory>,
    display_width: u32,
    display_height: u32,
    display_framebuffer: &'static RefCell<&'static mut [u8]>,
    display_cap: Capability<Display>,
}

impl WindowingServer for Server {
    fn create_window(&self, sender: ProcessId) -> Capability<Window> {
        let width = 400;
        let height = 300;
        let window_framebuffer_cap =
            unsafe { syscall::allocate_shared_memory(width as usize * height as usize * 4) };
        let (framebuffer_ptr, framebuffer_len) =
            unsafe { syscall::map_shared_memory(window_framebuffer_cap) };
        let window_framebuffer_data =
            unsafe { core::slice::from_raw_parts_mut(framebuffer_ptr, framebuffer_len) };
        let data = Box::leak(Box::new(WindowData {
            x: 0,
            y: 0,
            width,
            height,
            window_framebuffer_data,
            window_framebuffer_cap,
            display_width: self.width,
            display_height: self.height,
            display_framebuffer: self.framebuffer,
            display_cap: self.cap,
        }));
        grant_capability2(sender, data)
    }
}

impl WindowServer for WindowData {
    fn width(&self, _: ProcessId) -> u32 {
        self.width
    }

    fn height(&self, _: ProcessId) -> u32 {
        self.height
    }

    fn framebuffer(&self, sender: ProcessId) -> Capability<SharedMemory> {
        forward_capability_by_pid(self.window_framebuffer_cap, sender)
    }

    fn draw(&self, _: ProcessId) {
        let mut display_framebuffer = self.display_framebuffer.borrow_mut();
        for window_y in 0..self.height as usize {
            let display_y = self.y as usize + window_y;
            for window_x in 0..self.width as usize {
                let display_x = self.x as usize + window_x;
                for channel in 0..4 {
                    display_framebuffer
                        [display_y * 4 * self.display_width as usize + display_x * 4 + channel] =
                        self.window_framebuffer_data
                            [window_y * 4 * self.width as usize + window_x * 4 + channel];
                }
            }
        }
        self.display_cap.draw();
    }
}

unsafe impl Send for Server {}
unsafe impl Sync for Server {}
unsafe impl Send for WindowData {}
unsafe impl Sync for WindowData {}

fn main(args: Args) {
    let width = args.display.width();
    let height = args.display.height();
    debug!("found a {width}x{height} display");
    let framebuffer = args.display.framebuffer();
    let (framebuffer, framebuffer_len) = unsafe { syscall::map_shared_memory(framebuffer) };
    let framebuffer = unsafe { core::slice::from_raw_parts_mut(framebuffer, framebuffer_len) };
    fill_screen(191, 215, 234, framebuffer, &args);
    let server = Box::leak(Box::new(Server {
        width,
        height,
        framebuffer: Box::leak(Box::new(RefCell::new(framebuffer))),
        cap: args.display,
    }));
    register_root_capability(server);

    let start_time = riscv::register::time::read();
    let timebase_frequency = unsafe { syscall::riscv_timebase_frequency() } as f64;
    let mut keyboard = args.keyboard.events();
    loop {
        let time = (riscv::register::time::read() - start_time) as f64 / timebase_frequency;
        while let Some(event) = keyboard.next() {
            if event.type_ != 0 {
                debug!("{event:?} at time {time:.02}s");
            }
        }
        ipc_serve();
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
