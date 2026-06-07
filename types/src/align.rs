#[repr(align(64))]
pub struct CacheLineAligned<T: ?Sized>(pub T);

pub const CACHE_LINE_SIZE: usize = 64;
