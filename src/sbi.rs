use core::arch::asm;

pub macro console_writeln($($arg:tt)*) {
    core::fmt::write(&mut crate::sbi::SbiConsole, format_args!("{}\n", format_args!($($arg)*))).unwrap()
}

pub struct SbiConsole;

struct SbiResult {
    error: u64,
    value: u64,
}

impl core::fmt::Write for SbiConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            unsafe { sbi_call(byte as u64, 0, 0, 0, 0, 0, 0, 1) };
        }
        Ok(())
    }
}

unsafe fn sbi_call(
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    fid: u64,
    eid: u64,
) -> SbiResult {
    let error;
    let value;
    unsafe {
        asm!(
            "ecall",
            in("a0") arg0,
            in("a1") arg1,
            in("a2") arg2,
            in("a3") arg3,
            in("a4") arg4,
            in("a5") arg5,
            in("a6") fid,
            in("a7") eid,
            lateout("a0") error,
            lateout("a1") value,
        )
    }
    SbiResult { error, value }
}
