use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use deravel_types::{CacheLineAligned, PAGE_SIZE, RingBufferState};

pub struct RingBuffer<T> {
    data: Vec<T>,
    state: Box<RingBufferState>,
}

impl<T: Copy> RingBuffer<T> {
    pub fn new() -> RingBuffer<T> {
        let data = vec![unsafe { core::mem::zeroed() }; PAGE_SIZE / size_of::<T>()];
        let state = Box::new(RingBufferState {
            read: CacheLineAligned(AtomicUsize::new(0)),
            written: CacheLineAligned(AtomicUsize::new(0)),
        });
        RingBuffer { data, state }
    }

    pub fn push(&mut self, value: T) {
        let written = self.state.written.0.load(Ordering::Relaxed);
        let read = self.state.read.0.load(Ordering::Acquire);
        assert!(written < read + self.data.len());
        self.data[written] = value;
        self.state.written.0.store(written + 1, Ordering::Release);
    }

    pub fn expose(&self) -> (*mut u8, usize, *mut RingBufferState) {
        (
            self.data.as_ptr() as *mut u8,
            self.data.len() * size_of::<T>(),
            self.state.as_ref() as *const _ as *mut _,
        )
    }
}
