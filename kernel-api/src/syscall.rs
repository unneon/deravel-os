use core::arch::asm;
use core::hint::unreachable_unchecked;
use core::mem::transmute_copy;
use deravel_types::ProcessId;
use deravel_types::capability::Capability;

macro syscalls(
    $(#[no = $no:literal] pub fn $name:ident(
        $($a0name:ident: $a0type:ty
        $(, $a1name:ident: $a1type:ty
        $(, $a2name:ident: $a2type:ty
        $(, $a3name:ident: $a3type:ty
        $(, $a4name:ident: $a4type:ty
        $(, $a5name:ident: $a5type:ty
        )?)?)?)?)?)?
    ) $(-> $return_type:ty)?;)*
) {
    $(pub fn $name(
        $($a0name: $a0type
        $(, $a1name: $a1type
        $(, $a2name: $a2type
        $(, $a3name: $a3type
        $(, $a4name: $a4type
        $(, $a5name: $a5type
    )?)?)?)?)?)?) $(-> $return_type)? {
        let a0: usize;
        let a1: usize;
        let a2: usize;
        let a3: usize;
        unsafe {
            asm!(
                "ecall",
                $(in("a0") to_arg($a0name),
                $(in("a1") to_arg($a1name),
                $(in("a2") to_arg($a2name),
                $(in("a3") to_arg($a3name),
                $(in("a4") to_arg($a4name),
                $(in("a5") to_arg($a5name),
                )?)?)?)?)?)?
                in("a6") $no,
                lateout("a0") a0,
                lateout("a1") a1,
                lateout("a2") a2,
                lateout("a3") a3,
            );
            FromRet::from_ret(a0, a1, a2, a3)
        }
    })*
}

unsafe trait FromRet: Sized {
    unsafe fn from_ret(a0: usize, _a1: usize, _a2: usize, _a3: usize) -> Self {
        unsafe { transmute_copy(&a0) }
    }
}

union Register<T: Copy> {
    rust: T,
    register: usize,
}

unsafe impl FromRet for ! {}

unsafe impl FromRet for () {}

unsafe impl FromRet for u8 {}

unsafe impl FromRet for usize {}

unsafe impl<T> FromRet for *const T {}

unsafe impl<T> FromRet for *mut T {}

unsafe impl FromRet for Capability {}

unsafe impl FromRet for ProcessId {}

unsafe impl<A: FromRet, B: FromRet> FromRet for (A, B) {
    unsafe fn from_ret(a0: usize, a1: usize, _: usize, _: usize) -> (A, B) {
        unsafe {
            (
                FromRet::from_ret(a0, 0, 0, 0),
                FromRet::from_ret(a1, 0, 0, 0),
            )
        }
    }
}

unsafe impl<A: FromRet, B: FromRet, C: FromRet> FromRet for (A, B, C) {
    unsafe fn from_ret(a0: usize, a1: usize, a2: usize, _: usize) -> (A, B, C) {
        unsafe {
            (
                FromRet::from_ret(a0, 0, 0, 0),
                FromRet::from_ret(a1, 0, 0, 0),
                FromRet::from_ret(a2, 0, 0, 0),
            )
        }
    }
}

unsafe impl<A: FromRet, B: FromRet, C: FromRet, D: FromRet> FromRet for (A, B, C, D) {
    unsafe fn from_ret(a0: usize, a1: usize, a2: usize, a3: usize) -> (A, B, C, D) {
        unsafe {
            (
                FromRet::from_ret(a0, 0, 0, 0),
                FromRet::from_ret(a1, 0, 0, 0),
                FromRet::from_ret(a2, 0, 0, 0),
                FromRet::from_ret(a3, 0, 0, 0),
            )
        }
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
    pub fn pid_by_name(name: *const u8, name_len: usize) -> ProcessId;

    #[no = 8]
    pub fn log(text: *const u8, text_len: usize, level: usize);

    #[no = 9]
    pub fn disk_read(sector: usize, buf: *mut [u8; 512]);

    #[no = 10]
    pub fn disk_write(sector: usize, buf: *const [u8; 512]);

    #[no = 11]
    pub fn disk_capacity() -> usize;

    #[no = 12]
    pub fn allocate_pages(count: usize) -> *mut u8;

    #[no = 13]
    pub fn ipc_call(cap: Capability, method: usize, args: *const u8, args_len: usize, result: *mut u8, result_max_len: usize) -> usize;

    #[no = 14]
    pub fn ipc_receive(args: *mut u8, args_max_len: usize) -> (Capability, usize, usize, ProcessId);

    #[no = 15]
    pub fn ipc_reply(result: *const u8, result_len: usize);
}

unsafe fn to_arg<T: Copy>(rust: T) -> usize {
    unsafe { Register { rust }.register }
}
