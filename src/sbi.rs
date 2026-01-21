use core::arch::asm;
use core::mem::transmute;
use core::num::NonZeroIsize;

pub macro console_writeln($($arg:tt)*) {
    core::fmt::write(&mut crate::sbi::Console, format_args!("{}\n", format_args!($($arg)*))).unwrap()
}

macro function {
    (#[eid = $eid:expr, fid = $fid:expr] $(#[$($docs:tt)*])* $name:ident () $ret:ty) => {
        #[allow(dead_code)]
        $(#[$($docs)*])*
        pub fn $name() -> $ret {
            let error: isize;
            let value: usize;
            unsafe { asm!("ecall", in("a6") $fid, in("a7") $eid, lateout("a0") error, lateout("a1") value) }
            <$ret>::from_sbiret(error, value)
        }
    },
    (#[legacy, eid = $eid:expr] $(#[$($docs:tt)*])* $name:ident ($arg0:ident : $arg0type:ty) LegacyResult) => {
        #[allow(dead_code)]
        $(#[$($docs)*])*
        pub fn $name($arg0: $arg0type) -> LegacyResult {
            let error: isize;
            unsafe { asm!("ecall", in("a0") $arg0, in("a7") $eid, lateout("a0") error ) }
            unsafe { transmute(error) }
        }
    },
}

macro functions($(#[$($metadata:tt)*] $(#[$($docs:tt)*])* pub fn $name:ident $args:tt -> $ret:tt;)*) {
    $(function!(#[$($metadata)*] $(#[$($docs)*])* $name $args $ret);)*
}

trait FromSbiret {
    fn from_sbiret(error: isize, value: usize) -> Self;
}

pub struct Console;

pub struct ImplId(usize);

pub struct LegacyError(#[allow(dead_code)] pub NonZeroIsize);

pub type LegacyResult = Result<(), LegacyError>;

pub struct SpecVersion(u32);

impl ImplId {
    #[allow(dead_code)]
    pub fn number(&self) -> usize {
        self.0
    }

    pub fn name(&self) -> Option<&'static str> {
        const KNOWN_NAMES: [&str; 12] = [
            "Berkeley Boot Loader (BBL)",
            "OpenSBI",
            "Xvisor",
            "KVM",
            "RustSBI",
            "Diosix",
            "Coffer",
            "Xen Project",
            "PolarFire Hart Software Services",
            "coreboot",
            "oreboot",
            "bhyve",
        ];
        KNOWN_NAMES.get(self.0).copied()
    }
}

impl SpecVersion {
    pub fn minor(&self) -> u32 {
        self.0 & ((1 << 24) - 1)
    }

    pub fn major(&self) -> u32 {
        (self.0 >> 24) & ((1 << 7) - 1)
    }
}

impl core::fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            sbi_console_putchar(byte as u64).map_err(|_| core::fmt::Error)?;
        }
        Ok(())
    }
}

impl FromSbiret for usize {
    fn from_sbiret(_: isize, value: usize) -> Self {
        value
    }
}

impl FromSbiret for ImplId {
    fn from_sbiret(_: isize, value: usize) -> Self {
        ImplId(value)
    }
}

impl FromSbiret for SpecVersion {
    fn from_sbiret(_: isize, value: usize) -> Self {
        SpecVersion(value as u32)
    }
}

functions! {
    #[eid = 0x10, fid = 0]
    /// Returns the current SBI specification version.
    pub fn sbi_get_spec_version() -> SpecVersion;

    #[eid = 0x10, fid = 1]
    /// Returns the current SBI implementation ID, which is different for every SBI implementation. It is intended
    /// that this implementation ID allows software to probe for SBI implementation quirks.
    pub fn sbi_get_impl_id() -> ImplId;

    #[eid = 0x10, fid = 2]
    /// Returns the current SBI implementation version. The encoding of this version number is specific to the
    /// SBI implementation.
    pub fn sbi_get_impl_version() -> usize;

    #[legacy, eid = 0x01]
    /// Write data present in ch to debug console.
    ///
    /// Unlike sbi_console_getchar(), this SBI call will block if there remain any pending characters to be
    /// transmitted or if the receiving terminal is not yet ready to receive the byte. However, if the console doesn’t
    /// exist at all, then the character is thrown away.
    ///
    /// This SBI call may return an implementation specific negative error code.
    pub fn sbi_console_putchar(ch: u64) -> LegacyResult;
}
