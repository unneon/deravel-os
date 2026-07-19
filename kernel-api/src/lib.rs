#![allow(clippy::too_many_arguments)]
#![feature(decl_macro)]
#![feature(ptr_metadata)]
#![feature(slice_ptr_get)]
#![no_std]

extern crate alloc;

mod capability;
mod dispatch;
pub mod drvli;
mod framebuffer;

pub use capability::*;
pub use deravel_types::*;
pub use dispatch::*;
pub use drvli::*;
pub use framebuffer::Framebuffer;

use alloc::string::String;
use core::alloc::{GlobalAlloc, Layout};
use core::fmt::Write;
use core::sync::atomic::{AtomicUsize, Ordering};
use deravel_types::memory::USER_INPUTS;
use log::*;
use serde::Deserialize;

#[macro_export]
macro_rules! app {
    ($main:ident $name:ident) => {
        type Args = <$name as ProcessTag>::Args;

        #[unsafe(no_mangle)]
        extern "C" fn _start() -> ! {
            log::set_logger(&$crate::KernelLogger).unwrap();
            log::set_max_level(log::LevelFilter::Trace);
            $main(unsafe {
                (memory::USER_INPUTS.start as *const ProcessInputs<$name>)
                    .read()
                    .args
            });
            deravel_kernel_api::exit()
        }
    };
}

pub macro print($($tt:tt)*) {
    core::fmt::write(&mut Stdio, format_args!("{}", format_args!($($tt)*))).unwrap()
}

pub macro println {
    () => {
        print!("\n")
    },
    ($($tt:tt)*) => {
        print!("{}\n", format_args!($($tt)*))
    },
}

struct FakeProcess;

#[derive(Debug, Deserialize)]
struct FakeProcessArgs;

pub struct Stdio;

struct PageAllocator;

#[doc(hidden)]
pub struct KernelLogger;

#[global_allocator]
static PAGE_ALLOCATOR: PageAllocator = PageAllocator;

static STDIO: AtomicUsize = AtomicUsize::new(0);

impl ProcessTag for FakeProcess {
    type Args = FakeProcessArgs;
    type Export = ();
    type Spawner = ();
    const NAME: &'static str = "";
}

impl ProcessArgs for FakeProcessArgs {
    fn for_all(&self, _: impl FnMut(RawCapability)) {}
}

impl Write for Stdio {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let stdio = stdio();
        for byte in s.bytes() {
            stdio.putchar(byte);
        }
        Ok(())
    }
}

unsafe impl GlobalAlloc for PageAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        assert!(layout.align() <= PAGE_SIZE);
        unsafe { syscall::alloc(layout.size()) }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

impl log::Log for KernelLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let mut text = String::new();
        write!(text, "{}", record.args()).unwrap();
        let level = match record.level() {
            Level::Error => 0,
            Level::Warn => 1,
            Level::Info => 2,
            Level::Debug => 3,
            Level::Trace => 4,
        };
        unsafe { syscall::log(text.as_ptr(), text.len(), level) }
    }

    fn flush(&self) {}
}

pub fn alloc_shared(size: usize) -> (*mut [u8], Capability<SharedMemory>) {
    // TODO: This API should warn size gets rounded up to page size?
    let (ptr, cap) = unsafe { syscall::alloc_shared(size) };
    (core::ptr::slice_from_raw_parts_mut(ptr, size), cap)
}

pub fn current_pid() -> ProcessId {
    common_inputs().id
}

pub fn exit() -> ! {
    unsafe { syscall::exit() }
}

pub fn getchar() -> u8 {
    stdio().getchar()
}

pub fn map_shared(cap: Capability<SharedMemory>) -> *mut [u8] {
    let (pointer, size) = unsafe { syscall::map_shared(cap) };
    core::ptr::from_raw_parts_mut(pointer, size)
}

pub fn putchar(ch: u8) {
    stdio().putchar(ch)
}

pub fn set_stdio(cap: Capability<Console>) {
    STDIO.store(cap.as_usize(), Ordering::SeqCst);
}

pub fn system_time() -> f64 {
    riscv::register::time::read() as f64 / common_inputs().riscv_timebase_frequency.unwrap() as f64
}

pub fn yield_() {
    unsafe { syscall::yield_() }
}

fn common_inputs() -> &'static ProcessInputs<FakeProcess> {
    unsafe { &*(USER_INPUTS.start as *const ProcessInputs<FakeProcess>) }
}

fn stdio() -> Capability<Console> {
    let stdio = STDIO.load(Ordering::SeqCst) as *const CapabilityCertificate;
    assert!(!stdio.is_null(), "standard input/output not set");
    unsafe { Capability::new(RawCapability::try_from(stdio).unwrap()) }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let location = info.location().unwrap();
    let message = info.message();
    error!("user application panicked at {location}: {message}");
    exit()
}
