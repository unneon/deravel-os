use core::ops::{Deref, DerefMut};

#[repr(align(4096))]
pub struct PageAligned<T>(T);

pub const PAGE_SIZE: usize = 4096;

impl<T> Deref for PageAligned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for PageAligned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
