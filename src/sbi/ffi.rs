use crate::sbi::Error;
use core::arch::asm;
use core::mem::transmute;

macro function(#[eid = $eid:expr, fid = $fid:expr] $name:ident ($($a0name:ident: $a0type:ty $(, $a1name:ident: $a1type:ty $(, $a2name:ident: $a2type:ty)?)?)?) $ret:ty) {
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

macro functions($($(#[$($docs:tt)*])* pub fn $name:ident $args:tt -> $ret:ty;)*) {
    $(function!($(#[$($docs)*])* $name $args $ret);)*
}

trait FromSbiret {
    fn from_sbiret(error: isize, value: usize) -> Self;
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

impl FromSbiret for Result<!, Error> {
    fn from_sbiret(error: isize, _: usize) -> Self {
        Err(unsafe { transmute::<isize, Error>(error) })
    }
}

functions! {
    #[eid = 0x10, fid = 0]
    pub fn sbi_get_spec_version() -> usize;

    #[eid = 0x10, fid = 1]
    pub fn sbi_get_impl_id() -> usize;

    #[eid = 0x10, fid = 2]
    pub fn sbi_get_impl_version() -> usize;

    #[eid = 0x4442434E, fid = 0]
    pub fn sbi_debug_console_write(num_bytes: usize, base_addr_lo: usize, base_addr_hi: usize) -> Result<usize, Error>;

    #[eid = 0x4442434E, fid = 1]
    pub fn sbi_debug_console_read(num_bytes: usize, base_addr_lo: usize, base_addr_hi: usize) -> Result<usize, Error>;

    #[eid = 0x53525354, fid = 0]
    pub fn sbi_system_reset(reset_type: u32, reset_reason: u32) -> Result<!, Error>;
}
