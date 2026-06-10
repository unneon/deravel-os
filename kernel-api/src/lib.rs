#![feature(decl_macro)]
#![feature(never_type)]
#![feature(ptr_metadata)]
#![no_std]

extern crate alloc;

mod capability;
pub mod drvli;
pub mod syscall;

pub use capability::*;
pub use deravel_types::*;
pub use drvli::*;

use alloc::string::String;
use core::alloc::{GlobalAlloc, Layout};
use core::fmt::Write;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering};
use log::{Level, LevelFilter, Metadata, Record, error};

#[macro_export]
macro_rules! app {
    ($main:ident $name:ident) => {
        type Args = <$name as ProcessTag>::Args;

        #[unsafe(no_mangle)]
        extern "C" fn __deravel_main() -> ! {
            $main(unsafe { (INPUTS_ADDRESS as *const ProcessInputs<$name>).read().args });
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

pub struct Stdio;

struct PageAllocator;

struct KernelLogger;

unsafe extern "C" {
    static mut __deravel_stack_top: u8;
}

#[global_allocator]
static PAGE_ALLOCATOR: PageAllocator = PageAllocator;

static STDIO: AtomicUsize = AtomicUsize::new(0);

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
        let page_count = layout.size().div_ceil(PAGE_SIZE);
        unsafe { syscall::allocate_pages(page_count) }
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

pub fn allocate_shared_memory(size: usize) -> Capability<SharedMemory> {
    unsafe { syscall::allocate_shared_memory(size) }
}

pub fn exit() -> ! {
    unsafe { syscall::exit() }
}

pub fn getchar() -> u8 {
    stdio().getchar()
}

pub fn ipc_serve<S>(dispatch: &mut Dispatch<S>) {
    loop {
        let mut buf = [0u8; 4096];
        let (cap, method, args_len, sender) =
            unsafe { syscall::ipc_receive_async(buf.as_mut_ptr(), buf.len()) };
        if cap.as_usize() == 0 {
            break;
        }
        let result = dispatch.dispatch(cap, method, &buf[..args_len], sender);
        unsafe { syscall::ipc_reply(result.as_ptr(), result.len()) }
    }
    dispatch.run_observables();
}

pub fn map_shared_memory(cap: Capability<SharedMemory>) -> *mut [u8] {
    let (pointer, size) = unsafe { syscall::map_shared_memory(cap) };
    core::ptr::from_raw_parts_mut(pointer, size)
}

pub fn putchar(ch: u8) {
    stdio().putchar(ch)
}

pub fn set_stdio(cap: Capability<Console>) {
    STDIO.store(cap.as_usize(), Ordering::SeqCst);
}

pub fn system_time() -> f64 {
    riscv::register::time::read() as f64
        / unsafe {
            (INPUTS_ADDRESS as *const ProcessInputs<Hello>)
                .read()
                .riscv_timebase_frequency
        }
}

pub fn yield_() {
    unsafe { syscall::yield_() }
}

fn stdio() -> Capability<Console> {
    let stdio = STDIO.load(Ordering::SeqCst) as *mut CapabilityCertificate;
    assert!(!stdio.is_null(), "standard input/output not set");
    Capability(RawCapability::from_pointer(stdio), PhantomData)
}

fn current_pid() -> ProcessId {
    unsafe { (INPUTS_ADDRESS as *const ProcessInputs<Hello>).read().id }
}

fn initialize_log() {
    log::set_logger(&KernelLogger).unwrap();
    log::set_max_level(LevelFilter::Trace);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let location = info.location().unwrap();
    let message = info.message();
    error!("user application panicked at {location}: {message}");
    exit()
}
