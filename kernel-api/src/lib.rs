#![feature(decl_macro, never_type)]
#![no_std]

use core::arch::asm;
use core::hint::unreachable_unchecked;
use core::mem::{MaybeUninit, transmute_copy};
use core::sync::atomic::{AtomicUsize, Ordering};

pub macro app($main:ident) {
    unsafe extern "C" {
        static mut __deravel_stack_top: u8;
    }

    #[unsafe(naked)]
    #[unsafe(no_mangle)]
    unsafe extern "C" fn __deravel_entry() -> ! {
        core::arch::naked_asm!(
            "la sp, {stack_top}",
            "la t0, {pid}",
            "sd a0, 0(t0)",
            "call {main}",
            "call {exit}",
            stack_top = sym __deravel_stack_top,
            pid = sym PID,
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

#[derive(Clone, Copy)]
pub struct Capability(usize);

#[derive(Debug)]
pub enum CapabilityExport {
    Internal {
        dst_pid: usize,
    },
    Redirect {
        forwardee_pid: usize,
        inner: Capability,
    },
}

pub struct CapabilityExportPacked(#[allow(dead_code)] usize);

pub struct KernelConsole;

pub static CAPABILITIES_ALLOCATED: AtomicUsize = AtomicUsize::new(0);
pub static mut PID: usize = 0;

impl Capability {
    pub fn create(dst_pid: usize) -> Capability {
        let index = CAPABILITIES_ALLOCATED.fetch_add(1, Ordering::Relaxed);
        let address = 0x2000000 + 4096 * pid() + index * size_of::<usize>();
        let pointer = address as *mut CapabilityExportPacked;
        let internal = CapabilityExportPacked::internal(dst_pid);
        unsafe { pointer.write_volatile(internal) };
        Capability(address)
    }

    pub fn forward(&self, dst_pid: usize) -> Capability {
        let index = CAPABILITIES_ALLOCATED.fetch_add(1, Ordering::Relaxed);
        let address = 0x2000000 + 4096 * pid() + index * size_of::<usize>();
        let pointer = address as *mut CapabilityExportPacked;
        let redirect = CapabilityExportPacked::redirect(dst_pid, self);
        unsafe { pointer.write_volatile(redirect) };
        Capability(address)
    }

    pub fn validate_chain(&self, original_sender: usize) -> Capability {
        println!("validating capability {self:?} from process {original_sender}");
        let mut capability = *self;
        let mut sender = original_sender;
        let original = loop {
            match capability.read_export().unpack() {
                CapabilityExport::Internal { dst_pid } => {
                    println!(
                        "    ... originally sent from {} to {dst_pid}",
                        capability.src_pid()
                    );
                    assert_eq!(dst_pid, sender);
                    break capability;
                }
                CapabilityExport::Redirect {
                    forwardee_pid,
                    inner,
                } => {
                    println!(
                        "    ... was a forward of {inner:?} from {} to {forwardee_pid}",
                        capability.src_pid()
                    );
                    assert_eq!(forwardee_pid, sender);
                    sender = capability.src_pid();
                    capability = inner;
                }
            }
        };
        assert_eq!(original.src_pid(), pid());
        original
    }

    fn read_export(&self) -> CapabilityExportPacked {
        assert!((0x2000000..0x3000000).contains(&self.0));
        unsafe { (self.0 as *const CapabilityExportPacked).read_volatile() }
    }

    fn src_pid(&self) -> usize {
        (self.0 - 0x2000000) / 4096
    }
}

impl CapabilityExportPacked {
    fn internal(dst_pid: usize) -> CapabilityExportPacked {
        assert!(dst_pid < 8);
        CapabilityExportPacked(dst_pid)
    }

    fn redirect(dst_pid: usize, capability: &Capability) -> CapabilityExportPacked {
        assert!(dst_pid < 8);
        assert!(capability.0.is_multiple_of(8));
        CapabilityExportPacked(dst_pid + capability.0)
    }

    fn unpack(&self) -> CapabilityExport {
        if self.0 / 8 == 0 {
            CapabilityExport::Internal { dst_pid: self.0 }
        } else {
            let dst_pid = self.0 % 8;
            CapabilityExport::Redirect {
                forwardee_pid: dst_pid,
                inner: Capability(self.0 - dst_pid),
            }
        }
    }
}

impl core::fmt::Debug for Capability {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl core::fmt::Write for KernelConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            putchar(byte);
        }
        Ok(())
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
    pub fn raw_pid_by_name(name: *const u8, name_len: usize) -> usize;

    #[no = 6]
    pub fn raw_ipc_send(data: *const u8, data_len: usize, dest_pid: usize);

    #[no = 7]
    pub fn raw_ipc_recv(buf: *mut u8, buf_len: usize) -> usize;
}

pub fn ipc_send<T>(data: &T, dest_pid: usize) {
    raw_ipc_send(data as *const T as *const u8, size_of_val(data), dest_pid)
}

pub fn ipc_recv<T>() -> (T, usize) {
    let mut buf = MaybeUninit::<T>::uninit();
    let sender_pid = raw_ipc_recv(buf.as_mut_ptr() as *mut u8, size_of::<T>());
    (unsafe { buf.assume_init() }, sender_pid)
}

pub fn pid() -> usize {
    unsafe { PID }
}

pub fn pid_by_name(name: &str) -> usize {
    raw_pid_by_name(name.as_ptr(), name.len())
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let location = info.location().unwrap();
    let message = info.message();
    println!("\x1B[1;31muser application panicked\x1B[0m at {location}: {message}");
    loop {}
}
