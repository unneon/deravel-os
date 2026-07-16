use alloc::vec::Vec;
use core::ptr::NonNull;
use deravel_types::InvalidCapabilityError;

// TODO: How to handle unmapped user memory?
// Software page table traversal would kind of suck, touching everything would kind of suck. The
// performant-but-weird option would be to use the exception handler, and maybe either jump to some
// sort of kill-the-process-due-to-OOB procedure or set some flag and return a zero as the result of
// the load. Not like the performance of this handler particularly matters, given it's not ever
// expected to happen in a real system.

pub unsafe trait SafeUserType {}

pub struct UserPtr<T: ?Sized>(NonNull<T>);

pub struct UserPtrInvalid;

pub struct UserSliceTooSmall;

pub enum UserSyscallError {
    InvalidCapability(InvalidCapabilityError),
    PointerInvalid(UserPtrInvalid),
}

impl<T: SafeUserType> UserPtr<[T]> {
    pub fn from_slice(ptr: *mut T, len: usize) -> Result<UserPtr<[T]>, UserPtrInvalid> {
        if (ptr as usize) & (1 << (usize::BITS - 1)) != 0 {
            // This is not a user pointer, the condition works because of sign-extension.
            // TODO: Confirm using non-sign-extended pointer causes a page fault.
            return Err(UserPtrInvalid);
        }
        if !ptr.is_aligned() {
            return Err(UserPtrInvalid);
        }
        let ptr = core::ptr::slice_from_raw_parts_mut(ptr, len);
        let Some(ptr) = NonNull::new(ptr) else {
            return Err(UserPtrInvalid);
        };
        Ok(UserPtr(ptr))
    }

    pub fn copy(&self) -> Vec<T> {
        let mut kernel = Vec::with_capacity(self.0.len());
        unsafe {
            core::ptr::copy_nonoverlapping(self.0.as_mut_ptr(), kernel.as_mut_ptr(), self.0.len());
            kernel.set_len(self.0.len());
        }
        kernel
    }

    pub fn write(&mut self, data: &[T]) -> Result<(), UserSliceTooSmall> {
        if self.0.len() < data.len() {
            return Err(UserSliceTooSmall);
        }
        // TODO: Check if we're not writing into some other process' capability page.
        unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), self.0.as_mut_ptr(), data.len()) }
        Ok(())
    }
}

unsafe impl SafeUserType for u8 {}

impl From<InvalidCapabilityError> for UserSyscallError {
    fn from(err: InvalidCapabilityError) -> UserSyscallError {
        UserSyscallError::InvalidCapability(err)
    }
}

impl From<UserPtrInvalid> for UserSyscallError {
    fn from(err: UserPtrInvalid) -> UserSyscallError {
        UserSyscallError::PointerInvalid(err)
    }
}

impl core::fmt::Display for UserPtrInvalid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid user pointer")
    }
}

impl core::fmt::Display for UserSliceTooSmall {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "user slice too small")
    }
}

impl core::fmt::Display for UserSyscallError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            UserSyscallError::InvalidCapability(err) => err.fmt(f),
            UserSyscallError::PointerInvalid(err) => err.fmt(f),
        }
    }
}
