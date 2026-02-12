#![feature(decl_macro)]
#![feature(never_type)]
#![feature(pointer_is_aligned_to)]
#![no_std]

mod capability;

pub use capability::*;

use core::alloc::{GlobalAlloc, Layout};
use core::arch::asm;
use core::fmt::Write;
use core::hint::unreachable_unchecked;
use core::mem::transmute_copy;
use log::{Level, LevelFilter, Metadata, Record, error};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub macro app($main:ident) {
    #[unsafe(no_mangle)]
    extern "C" fn __deravel_main() -> ! {
        $main();
        exit()
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

macro syscalls($(#[no = $no:literal] pub fn $name:ident($($a0name:ident: $a0type:ty$(, $a1name:ident: $a1type:ty$(, $a2name:ident: $a2type:ty)?)?)?) $(-> $return_type:ty)?;)*) {
    $(pub fn $name($($a0name: $a0type$(, $a1name: $a1type$(, $a2name: $a2type)?)?)?) $(-> $return_type)? {
        let _a0: usize;
        let _a1: usize;
        unsafe {
            asm!(
                "ecall",
                $(in("a0") <$a0type as To1A>::to_1a($a0name),
                $(in("a1") <$a1type as To1A>::to_1a($a1name),
                $(in("a2") <$a2type as To1A>::to_1a($a2name),
                )?)?)?
                in("a3") $no,
                lateout("a0") _a0,
                lateout("a1") _a1,
            )
        }
        $(<$return_type as FromA0A1>::from_a0a1(_a0, _a1))?
    })*
}

trait FromA0A1 {
    fn from_a0a1(a0: usize, a1: usize) -> Self;
}

trait To1A {
    fn to_1a(self) -> usize;
}

pub struct KernelConsole;

struct PageAllocator;

#[repr(transparent)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ProcessId(usize);

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
            putchar(byte);
        }
        Ok(())
    }
}

impl core::fmt::Debug for ProcessId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
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
        allocate_pages(page_count)
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

impl FromA0A1 for u8 {
    fn from_a0a1(a0: usize, _: usize) -> u8 {
        debug_assert!(a0 <= u8::MAX as usize);
        unsafe { transmute_copy::<usize, u8>(&a0) }
    }
}

impl FromA0A1 for usize {
    fn from_a0a1(a0: usize, _: usize) -> usize {
        a0
    }
}

impl FromA0A1 for *mut u8 {
    fn from_a0a1(a0: usize, _: usize) -> Self {
        a0 as *mut u8
    }
}

impl FromA0A1 for ProcessId {
    fn from_a0a1(a0: usize, _: usize) -> ProcessId {
        ProcessId(a0)
    }
}

impl FromA0A1 for (usize, ProcessId) {
    fn from_a0a1(a0: usize, a1: usize) -> Self {
        (a0, ProcessId(a1))
    }
}

impl FromA0A1 for (Capability, ProcessId) {
    fn from_a0a1(a0: usize, a1: usize) -> Self {
        (Capability(a0 as *const _), ProcessId(a1))
    }
}

impl FromA0A1 for ! {
    fn from_a0a1(_: usize, _: usize) -> ! {
        unsafe { unreachable_unchecked() }
    }
}

impl To1A for u8 {
    fn to_1a(self) -> usize {
        self as usize
    }
}

impl To1A for usize {
    fn to_1a(self) -> usize {
        self
    }
}

impl<T> To1A for *const T {
    fn to_1a(self) -> usize {
        self as usize
    }
}

impl<T> To1A for *mut T {
    fn to_1a(self) -> usize {
        self as usize
    }
}

impl<T> To1A for &T {
    fn to_1a(self) -> usize {
        self as *const T as usize
    }
}

impl<T> To1A for &mut T {
    fn to_1a(self) -> usize {
        self as *mut T as usize
    }
}

impl To1A for Capability {
    fn to_1a(self) -> usize {
        self.0 as usize
    }
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

syscalls! {
    #[no = 1]
    pub fn exit() -> !;

    #[no = 2]
    pub fn putchar(ch: u8);

    #[no = 3]
    pub fn getchar() -> u8;

    #[no = 4]
    pub fn yield_();

    #[no = 5]
    pub fn raw_pid_by_name(name: *const u8, name_len: usize) -> ProcessId;

    #[no = 6]
    pub fn raw_ipc_send(data: *const u8, data_len: usize, dest: usize);

    #[no = 7]
    pub fn raw_ipc_recv(buf: *mut u8, buf_max_len: usize) -> (usize, ProcessId);

    #[no = 8]
    pub fn raw_system_log(text: *const u8, text_len: usize, level: usize);

    #[no = 9]
    pub fn disk_read(sector: usize, buf: &mut [u8; 512]);

    #[no = 10]
    pub fn disk_write(sector: usize, buf: &[u8; 512]);

    #[no = 11]
    pub fn disk_capacity() -> usize;

    #[no = 12]
    pub fn allocate_pages(count: usize) -> *mut u8;
}

fn initialize_log() {
    log::set_logger(&SystemLogger).unwrap();
    log::set_max_level(LevelFilter::Trace);
}

pub fn ipc_send<T: Serialize>(data: &T, dest: ProcessId) {
    let buf = serde_json::to_vec(data).unwrap();
    raw_ipc_send(buf.as_ptr(), buf.len(), dest.0)
}

pub fn ipc_recv<T: DeserializeOwned>() -> (T, ProcessId) {
    let mut buf = [0; 1024];
    let (byte_count, sender_pid) = raw_ipc_recv(buf.as_mut_ptr(), buf.len());
    let value = serde_json::from_slice(&buf[..byte_count]).unwrap();
    (value, sender_pid)
}

pub fn current_pid() -> ProcessId {
    unsafe { CURRENT_PID }
}

pub fn pid_by_name(name: &str) -> ProcessId {
    raw_pid_by_name(name.as_ptr(), name.len())
}

pub fn system_log(text: &str, level: usize) {
    raw_system_log(text.as_ptr(), text.len(), level);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let location = info.location().unwrap();
    let message = info.message();
    error!("user application panicked at {location}: {message}");
    exit()
}
