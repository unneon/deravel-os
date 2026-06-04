#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec::Vec;
use deravel_kernel_api::*;
use log::debug;

struct Server {
    windows: Vec<WindowData>,
}

struct WindowData {}

impl WindowingServer for Server {
    fn create_window(&mut self, _: Capability<Windowing>, sender: ProcessId) -> Capability<Window> {
        self.windows.push(WindowData {});
        grant_capability(sender)
    }
}

fn main(args: Args) {
    let width = args.display.width() as usize;
    let height = args.display.height() as usize;
    debug!("found a {width}x{height} display");
    let framebuffer = args.display.framebuffer();
    let (framebuffer, framebuffer_len) = unsafe { syscall::map_shared_memory(framebuffer) };
    let framebuffer = unsafe { core::slice::from_raw_parts_mut(framebuffer, framebuffer_len) };
    let start_time = riscv::register::time::read();
    let timebase_frequency = unsafe { syscall::riscv_timebase_frequency() } as f64;
    let mut keyboard = args.keyboard.events();
    let mut last_switch = f64::NEG_INFINITY;
    let mut last_color = [255, 0, 0];
    let mut server = Server {
        windows: Vec::new(),
    };
    loop {
        let time = (riscv::register::time::read() - start_time) as f64 / timebase_frequency;
        while let Some(event) = keyboard.next() {
            if event.type_ != 0 {
                debug!("{event:?} at time {time:.02}s");
            }
        }
        ipc_serve_windowing_async(&mut server);
        if time - last_switch >= 0.5 {
            let [r, g, b] = last_color;
            fill_screen(b, g, r, framebuffer, &args);
            last_switch = time;
            last_color = [b, g, r];
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
