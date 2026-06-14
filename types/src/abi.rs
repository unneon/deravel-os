use crate::{Capability, ProcessId, RawCapability};

pub unsafe trait SyscallAbi: Copy + Sized {
    unsafe fn from_ret(register: usize, _: usize, _: usize, _: usize) -> Self {
        unsafe { Register { register }.rust }
    }
}

union Register<T: Copy> {
    rust: T,
    register: usize,
}

unsafe impl SyscallAbi for ! {}

unsafe impl SyscallAbi for () {}

unsafe impl SyscallAbi for u8 {}

unsafe impl SyscallAbi for u64 {}

unsafe impl SyscallAbi for usize {}

unsafe impl<T: ?Sized> SyscallAbi for &T {}

unsafe impl<T: ?Sized> SyscallAbi for *const T {}

unsafe impl<T: ?Sized> SyscallAbi for *mut T {}

unsafe impl<T> SyscallAbi for Capability<T> {}

unsafe impl SyscallAbi for RawCapability {}

unsafe impl SyscallAbi for Option<RawCapability> {}

unsafe impl SyscallAbi for ProcessId {}

unsafe impl SyscallAbi for Option<ProcessId> {}

pub unsafe fn from_reg<T: SyscallAbi>(register: usize) -> T {
    unsafe { Register { register }.rust }
}

pub unsafe fn to_reg<T: Copy>(rust: T) -> usize {
    unsafe { Register { rust }.register }
}
