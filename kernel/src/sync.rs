use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct Mutex<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

pub struct MutexGuard<'a, T> {
    locked: &'a AtomicBool,
    value: &'a mut T,
}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Mutex<T> {
        Mutex {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        lock(&self.locked);
        MutexGuard {
            locked: &self.locked,
            value: unsafe { &mut *self.value.get() },
        }
    }
}

impl<T> Mutex<Option<T>> {
    pub fn lock_if_some(&self) -> Option<MutexGuard<'_, T>> {
        lock(&self.locked);
        let value = unsafe { &mut *self.value.get() };
        if let Some(value) = value {
            Some(MutexGuard {
                locked: &self.locked,
                value,
            })
        } else {
            unlock(&self.locked);
            None
        }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        unlock(self.locked);
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}

unsafe impl<T: Send> Sync for Mutex<T> {}

fn lock(locked: &AtomicBool) {
    #[allow(clippy::never_loop)]
    while locked.swap(true, Ordering::Acquire) {
        panic!("deadlock detected as SMP not implemented yet");
    }
}

fn unlock(locked: &AtomicBool) {
    locked.store(false, Ordering::Release);
}
