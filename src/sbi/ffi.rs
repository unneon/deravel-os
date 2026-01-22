use core::arch::asm;
use core::mem::transmute;

macro function(#[eid = $eid:expr, fid = $fid:expr] $(#[$($docs:tt)*])* $name:ident ($($a0name:ident: $a0type:ty $(, $a1name:ident: $a1type:ty $(, $a2name:ident: $a2type:ty)?)?)?) $ret:ty) {
    $(#[$($docs)*])*
    pub fn $name($($a0name: $a0type,)? $($($a1name: $a1type,)?)? $($($($a2name: $a2type,)?)?)?) -> $ret {
        let error: isize;
        let value: usize;
        unsafe {
            asm!(
                "ecall",
                $(in("a0") $a0name,)?
                $($(in("a1") $a1name,)?)?
                $($($(in("a2") $a2name,)?)?)?
                in("a6") $fid,
                in("a7") $eid,
                lateout("a0") error,
                lateout("a1") value,
            )
        }
        <$ret>::from_sbiret(error, value)
    }
}

macro functions($(#[$($metadata:tt)*] $(#[$($docs:tt)*])* pub fn $name:ident $args:tt -> $ret:ty;)*) {
    $(function!(#[$($metadata)*] $(#[$($docs)*])* $name $args $ret);)*
}

trait FromSbiret {
    fn from_sbiret(error: isize, value: usize) -> Self;
}

#[allow(dead_code)]
#[derive(Debug)]
#[repr(isize)]
pub enum Error {
    /// Failed
    Failed = -1,
    /// Not supported
    NotSupported = -2,
    /// Invalid parameter(s)
    InvalidParam = -3,
    /// Denied or not allowed
    Denied = -4,
    /// Invalid address(s)
    InvalidAddress = -5,
    /// Already available
    AlreadyAvailable = -6,
    /// Already started
    AlreadyStarted = -7,
    /// Already stopped
    AlreadyStopped = -8,
    /// Shared memory not available
    NoShmem = -9,
    /// Invalid state
    InvalidState = -10,
    /// Bad (or invalid) range
    BadRange = -11,
    /// Failed due to timeout
    Timeout = -12,
    /// Input/Output error
    Io = -13,
    /// Denied or not allowed due to lock status
    DeniedLocked = -14,
}

impl FromSbiret for usize {
    fn from_sbiret(_: isize, value: usize) -> Self {
        value
    }
}

impl FromSbiret for Result<usize, Error> {
    fn from_sbiret(error: isize, value: usize) -> Self {
        if error == 0 {
            Ok(value)
        } else {
            Err(unsafe { transmute::<isize, Error>(error) })
        }
    }
}

functions! {
    #[eid = 0x10, fid = 0]
    /// Returns the current SBI specification version.
    pub fn sbi_get_spec_version() -> usize;

    #[eid = 0x10, fid = 1]
    /// Returns the current SBI implementation ID.
    ///
    /// It is intended that this implementation ID allows software to probe for SBI implementation quirks.
    pub fn sbi_get_impl_id() -> usize;

    #[eid = 0x10, fid = 2]
    /// Returns the current SBI implementation version.
    ///
    /// The encoding of this version number is specific to the SBI implementation.
    pub fn sbi_get_impl_version() -> usize;

    #[eid = 0x4442434E, fid = 0]
    /// Write bytes to the debug console from input memory.
    ///
    /// The `num_bytes` parameter specifies the number of bytes in the input memory. The physical base address of
    /// the input memory is represented by two XLEN bits wide parameters. The `base_addr_lo` parameter
    /// specifies the lower XLEN bits and the `base_addr_hi` parameter specifies the upper XLEN bits of the input
    /// memory physical base address.
    ///
    /// This is a non-blocking SBI call and it may do partial/no writes if the debug console is not able to accept
    /// more bytes.
    ///
    /// The number of bytes written is returned and possible errors are shown below.
    /// | Error code | Description |
    /// | ---------- | ----------- |
    /// | [`Error::InvalidParam`] | The memory pointed to by the `num_bytes`, `base_addr_lo`, and `base_addr_hi` parameters does not satisfy the requirements. |
    /// | [`Error::Denied`] | Writes to the debug console is not allowed. |
    /// | [`Error::Failed`] | Failed to write due to I/O errors. |
    pub fn sbi_debug_console_write(num_bytes: usize, base_addr_lo: usize, base_addr_hi: usize) -> Result<usize, Error>;
}
