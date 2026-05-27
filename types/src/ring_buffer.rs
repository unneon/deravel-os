use crate::CacheLineAligned;
use core::sync::atomic::AtomicUsize;

pub struct RingBufferState {
    pub read: CacheLineAligned<AtomicUsize>,
    pub written: CacheLineAligned<AtomicUsize>,
}
