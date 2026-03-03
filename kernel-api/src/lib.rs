#![feature(decl_macro)]
#![feature(never_type)]
#![feature(pointer_is_aligned_to)]
#![no_std]

mod capability;
mod syscall;

pub use capability::*;
pub use deravel_types::capability::Capability;
pub use syscall::{disk_capacity, disk_read, disk_write, getchar, putchar};

use core::alloc::{GlobalAlloc, Layout};
use core::fmt::Write;
use deravel_types::ProcessId;
use log::{Level, LevelFilter, Metadata, Record, error};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub macro app($main:ident) {
    #[unsafe(no_mangle)]
    extern "C" fn __deravel_main() -> ! {
        $main();
        deravel_kernel_api::syscall::exit()
    }
}

pub macro print($($tt:tt)*) {
    core::fmt::write(&mut KernelConsole, format_args!("{}", format_args!($($tt)*))).unwrap()
}

pub macro println {
    () => {
        print!("\n")
    },
    ($($tt:tt)*) => {
        print!("{}\n", format_args!($($tt)*))
    },
}

pub struct KernelConsole;

struct PageAllocator;

struct StackString {
    length: usize,
    buffer: [u8; 1024],
}

struct SystemLogger;

const PAGE_SIZE: usize = 4096;

unsafe extern "C" {
    static mut __deravel_stack_top: u8;
}

static mut CURRENT_PID: ProcessId = ProcessId(0);

#[global_allocator]
static PAGE_ALLOCATOR: PageAllocator = PageAllocator;

impl Write for KernelConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            syscall::putchar(byte);
        }
        Ok(())
    }
}

impl Write for StackString {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        assert!(s.len() <= self.buffer.len() - self.length);
        self.buffer[self.length..self.length + s.len()].copy_from_slice(s.as_bytes());
        self.length += s.len();
        Ok(())
    }
}

unsafe impl GlobalAlloc for PageAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        assert!(layout.align() <= PAGE_SIZE);
        let page_count = layout.size().div_ceil(PAGE_SIZE);
        syscall::allocate_pages(page_count)
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

impl log::Log for SystemLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let mut text = StackString {
            length: 0,
            buffer: [0; _],
        };
        write!(text, "{}", record.args()).unwrap();
        let level = match record.level() {
            Level::Error => 0,
            Level::Warn => 1,
            Level::Info => 2,
            Level::Debug => 3,
            Level::Trace => 4,
        };
        system_log(
            core::str::from_utf8(&text.buffer[..text.length]).unwrap(),
            level,
        )
    }

    fn flush(&self) {}
}

#[unsafe(link_section = ".text.entry")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn __deravel_entry() -> ! {
    core::arch::naked_asm!(
        "la sp, {stack_top}",
        "la t0, {current_pid}",
        "sd a0, 0(t0)",
        "call {initialize_log}",
        "j __deravel_main",
        stack_top = sym __deravel_stack_top,
        current_pid = sym CURRENT_PID,
        initialize_log = sym initialize_log,
    )
}

fn initialize_log() {
    log::set_logger(&SystemLogger).unwrap();
    log::set_max_level(LevelFilter::Trace);
}

pub fn ipc_send<T: Serialize + ?Sized>(data: &T, dest: ProcessId) {
    let buf = serde_json::to_vec(data).unwrap();
    syscall::ipc_send(buf.as_ptr(), buf.len(), dest.0)
}

pub fn ipc_recv<T: DeserializeOwned>() -> (T, ProcessId) {
    let mut buf = [0; 1024];
    let (byte_count, sender_pid) = syscall::ipc_recv(buf.as_mut_ptr(), buf.len());
    let value = serde_json::from_slice(&buf[..byte_count]).unwrap();
    (value, sender_pid)
}

pub fn current_pid() -> ProcessId {
    unsafe { CURRENT_PID }
}

pub fn pid_by_name(name: &str) -> ProcessId {
    syscall::pid_by_name(name.as_ptr(), name.len())
}

pub fn system_log(text: &str, level: usize) {
    syscall::log(text.as_ptr(), text.len(), level);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let location = info.location().unwrap();
    let message = info.message();
    error!("user application panicked at {location}: {message}");
    syscall::exit()
}
