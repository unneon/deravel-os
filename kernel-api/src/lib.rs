#![feature(decl_macro)]
#![feature(never_type)]
#![feature(pointer_is_aligned_to)]
#![no_std]

extern crate alloc;

mod capability;
pub mod drvli;
pub mod syscall;

pub use capability::*;
pub use deravel_types;
pub use deravel_types::capability::Capability;
pub use syscall::{disk_capacity, disk_read, disk_write, getchar, putchar};

use core::alloc::{GlobalAlloc, Layout};
use core::fmt::Write;
use deravel_types::ProcessId;
use log::{Level, LevelFilter, Metadata, Record, error};

#[macro_export]
macro_rules! app {
    ($main:ident $name:ident) => {
        type Args = <deravel_types::drvli::$name as deravel_kernel_api::drvli::App>::Args;

        #[unsafe(no_mangle)]
        extern "C" fn __deravel_main() -> ! {
            $main(unsafe {
                core::mem::transmute::<
                    <deravel_types::drvli::$name as deravel_types::drvli::ProcessTag>::Capabilities,
                    Args,
                >(
                    (deravel_types::INPUTS_ADDRESS
                        as *const deravel_types::ProcessInputs<deravel_types::drvli::$name>)
                        .read()
                        .args,
                )
            });
            deravel_kernel_api::exit()
        }
    };
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

#[global_allocator]
static PAGE_ALLOCATOR: PageAllocator = PageAllocator;

impl Write for KernelConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            unsafe { syscall::putchar(byte) }
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
        unsafe { syscall::allocate_pages(page_count) }
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
        "call {initialize_log}",
        "j __deravel_main",
        stack_top = sym __deravel_stack_top,
        initialize_log = sym initialize_log,
    )
}

fn initialize_log() {
    log::set_logger(&SystemLogger).unwrap();
    log::set_max_level(LevelFilter::Trace);
}

pub fn exit() -> ! {
    unsafe { syscall::exit() }
}

pub fn current_pid() -> ProcessId {
    unsafe {
        (deravel_types::INPUTS_ADDRESS
            as *const deravel_types::ProcessInputs<deravel_types::drvli::hello>)
            .read()
            .id
    }
}

pub fn system_log(text: &str, level: usize) {
    unsafe { syscall::log(text.as_ptr(), text.len(), level) }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let location = info.location().unwrap();
    let message = info.message();
    error!("user application panicked at {location}: {message}");
    exit()
}
