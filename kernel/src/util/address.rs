use core::ops::Range;

pub trait Address {
    type Raw;

    fn raw_addr(&self) -> Self::Raw;

    fn deep_map_addr(self, f: impl FnMut(usize) -> usize) -> Self;
}

impl<T> Address for *const T {
    type Raw = usize;

    fn raw_addr(&self) -> usize {
        *self as usize
    }

    fn deep_map_addr(self, f: impl FnMut(usize) -> usize) -> Self {
        self.map_addr(f)
    }
}

impl<T> Address for *mut T {
    type Raw = usize;

    fn raw_addr(&self) -> usize {
        *self as usize
    }

    fn deep_map_addr(self, f: impl FnMut(usize) -> usize) -> Self {
        self.map_addr(f)
    }
}

impl<T: Address> Address for Range<T> {
    type Raw = Range<T::Raw>;

    fn raw_addr(&self) -> Range<T::Raw> {
        self.start.raw_addr()..self.end.raw_addr()
    }

    fn deep_map_addr(self, mut f: impl FnMut(usize) -> usize) -> Self {
        self.start.deep_map_addr(&mut f)..self.end.deep_map_addr(f)
    }
}
