use core::sync::atomic::Ordering;
use deravel_types::RingBufferState;

pub struct UserRingBuffer<T> {
    pub(crate) data: *mut T,
    pub(crate) len: usize,
    // TODO: This should be something like a Box instead.
    pub(crate) state: *const RingBufferState,
}

impl<T: Copy> UserRingBuffer<T> {
    pub fn poll(&mut self) -> Option<T> {
        let state = unsafe { &*self.state };
        let read = state.read.0.load(Ordering::Relaxed);
        let written = state.written.0.load(Ordering::Acquire);
        if written <= read {
            return None;
        }
        let element = unsafe { self.data.add(read % self.len).read() };
        state.read.0.store(read + 1, Ordering::Release);
        Some(element)
    }
}
