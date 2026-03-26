use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct Mutex<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

pub struct MutexGuard<'a, T> {
    lock: &'a Mutex<T>,
}

impl<T> Mutex<T> {
    #[allow(dead_code)]
    pub const fn new(value: T) -> Mutex<T> {
        Mutex {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        while self.locked.swap(true, Ordering::Acquire) {}
        MutexGuard { lock: self }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.lock.value.as_ref_unchecked() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.lock.value.as_mut_unchecked() }
    }
}

unsafe impl<T: Send> Sync for Mutex<T> {}
