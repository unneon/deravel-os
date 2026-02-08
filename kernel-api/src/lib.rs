#![feature(decl_macro, never_type)]
#![no_std]

mod capability;

pub use capability::*;

use core::arch::asm;
use core::hint::unreachable_unchecked;
use core::mem::{MaybeUninit, transmute_copy};

pub macro app($main:ident) {
    unsafe extern "C" {
        static mut __deravel_stack_top: u8;
    }

    #[unsafe(naked)]
    #[unsafe(no_mangle)]
    unsafe extern "C" fn __deravel_entry() -> ! {
        core::arch::naked_asm!(
            "la sp, {stack_top}",
            "la t0, {current_pid}",
            "sd a0, 0(t0)",
            "call {main}",
            "call {exit}",
            stack_top = sym __deravel_stack_top,
            current_pid = sym CURRENT_PID,
            main = sym $main,
            exit = sym exit,
        )
    }
}

pub macro print($($tt:tt)*) {
    core::fmt::write(&mut KernelConsole, format_args!("{}", format_args!($($tt)*))).unwrap()
}

pub macro println($($tt:tt)*) {
    print!("{}\n", format_args!($($tt)*))
}

macro syscalls($(#[no = $no:literal] pub fn $name:ident($($a0name:ident: $a0type:ty$(, $a1name:ident: $a1type:ty$(, $a2name:ident: $a2type:ty)?)?)?) $(-> $return_type:ty)?;)*) {
    $(pub fn $name($($a0name: $a0type$(, $a1name: $a1type$(, $a2name: $a2type)?)?)?) $(-> $return_type)? {
        let _a0: usize;
        let _a1: usize;
        unsafe {
            asm!(
                "ecall",
                $(in("a0") $a0name,
                $(in("a1") $a1name,
                $(in("a2") $a2name,
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

pub struct KernelConsole;

#[repr(transparent)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ProcessId(usize);

pub static mut CURRENT_PID: ProcessId = ProcessId(0);

impl core::fmt::Write for KernelConsole {
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

impl FromA0A1 for ProcessId {
    fn from_a0a1(a0: usize, _: usize) -> ProcessId {
        ProcessId(a0)
    }
}

impl FromA0A1 for ! {
    fn from_a0a1(_: usize, _: usize) -> ! {
        unsafe { unreachable_unchecked() }
    }
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
    pub fn raw_ipc_recv(buf: *mut u8, buf_len: usize) -> ProcessId;
}

pub fn ipc_send<T>(data: &T, dest: ProcessId) {
    raw_ipc_send(data as *const T as *const u8, size_of_val(data), dest.0)
}

pub fn ipc_recv<T>() -> (T, ProcessId) {
    let mut buf = MaybeUninit::<T>::uninit();
    let sender_pid = raw_ipc_recv(buf.as_mut_ptr() as *mut u8, size_of::<T>());
    (unsafe { buf.assume_init() }, sender_pid)
}

pub fn current_pid() -> ProcessId {
    unsafe { CURRENT_PID }
}

pub fn pid_by_name(name: &str) -> ProcessId {
    raw_pid_by_name(name.as_ptr(), name.len())
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let location = info.location().unwrap();
    let message = info.message();
    println!("\x1B[1;31muser application panicked\x1B[0m at {location}: {message}");
    loop {}
}
