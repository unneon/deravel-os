use core::arch::asm;
use core::mem::transmute;

macro function {
    (#[eid = $eid:expr, fid = $fid:expr] $(#[$($docs:tt)*])* $name:ident () $ret:ty) => {
        $(#[$($docs)*])*
        pub fn $name() -> $ret {
            let error: isize;
            let value: usize;
            unsafe { asm!("ecall", in("a6") $fid, in("a7") $eid, lateout("a0") error, lateout("a1") value) }
            <$ret>::from_sbiret(error, value)
        }
    },
    (#[legacy, eid = $eid:expr] $(#[$($docs:tt)*])* $name:ident ($arg0:ident : $arg0type:ty) $ret:ty) => {
        $(#[$($docs)*])*
        pub fn $name($arg0: $arg0type) -> $ret {
            let error: isize;
            unsafe { asm!("ecall", in("a0") $arg0, in("a7") $eid, lateout("a0") error ) }
            unsafe { transmute(error) }
        }
    },
}

macro functions($(#[$($metadata:tt)*] $(#[$($docs:tt)*])* pub fn $name:ident $args:tt -> $ret:ty;)*) {
    $(function!(#[$($metadata)*] $(#[$($docs)*])* $name $args $ret);)*
}

trait FromSbiret {
    fn from_sbiret(error: isize, value: usize) -> Self;
}

// TODO: How to ensure Result<(), NegativeError> uses 0 as the Ok tag?
#[cfg_attr(
    target_pointer_width = "32",
    rustc_layout_scalar_valid_range_start(0x8000_0000)
)]
#[cfg_attr(
    target_pointer_width = "32",
    rustc_layout_scalar_valid_range_start(0xffff_ffff)
)]
#[cfg_attr(
    target_pointer_width = "64",
    rustc_layout_scalar_valid_range_start(0x8000_0000_0000_0000)
)]
#[cfg_attr(
    target_pointer_width = "64",
    rustc_layout_scalar_valid_range_end(0xffff_ffff_ffff_ffff)
)]
#[repr(transparent)]
#[derive(Debug)]
pub struct NegativeError {
    error: isize,
}

impl FromSbiret for usize {
    fn from_sbiret(_: isize, value: usize) -> Self {
        value
    }
}

functions! {
    #[eid = 0x10, fid = 0]
    /// Returns the current SBI specification version.
    pub fn sbi_get_spec_version() -> usize;

    #[eid = 0x10, fid = 1]
    /// Returns the current SBI implementation ID, which is different for every SBI implementation. It is intended
    /// that this implementation ID allows software to probe for SBI implementation quirks.
    pub fn sbi_get_impl_id() -> usize;

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
    pub fn sbi_console_putchar(ch: u64) -> Result<(), NegativeError>;
}
