#![feature(decl_macro, never_type)]
#![no_std]

use core::arch::asm;
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

macro syscalls($(#[no = $no:literal] pub fn $name:ident($($a0name:ident: $a0type:ty$(, $a1name:ident: $a1type:ty)?)?) $(-> $return_type:ty)?;)*) {
    $(pub fn $name($($a0name: $a0type$(, $a1name: $a1type)?)?) $(-> $return_type)? {
        let _result: u64;
        unsafe {
            asm!(
                "ecall",
                $(in("a0") $a0name,
                $(in("a1") $a1name,
                )?)?
                in("a3") $no,
                lateout("a0") _result,
            );
            $(core::mem::transmute_copy::<u64, $return_type>(&_result))?
        }
    })*
}

pub struct Capability(usize);

#[derive(Debug)]
pub enum CapabilityExport {
    Internal { dst_pid: usize },
    Redirect { dst_pid: usize, inner: Capability },
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

    pub fn guess(address: usize) -> Capability {
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

    pub fn read(&self) -> CapabilityExportPacked {
        unsafe { (self.0 as *const CapabilityExportPacked).read_volatile() }
    }

    pub fn src_pid(&self) -> usize {
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

    pub fn unpack(&self) -> CapabilityExport {
        if self.0 / 8 == 0 {
            CapabilityExport::Internal { dst_pid: self.0 }
        } else {
            let dst_pid = self.0 % 8;
            CapabilityExport::Redirect {
                dst_pid,
                inner: Capability::guess(self.0 - dst_pid),
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
