use core::ops::Range;

pub trait Address {
    type Raw;

    fn raw_addr(&self) -> Self::Raw;

    fn deep_map_addr(self, f: impl FnMut(usize) -> usize) -> Self;
}

impl Address for usize {
    type Raw = usize;

    fn raw_addr(&self) -> Self::Raw {
        *self
    }

    fn deep_map_addr(self, mut f: impl FnMut(usize) -> usize) -> Self {
        f(self)
    }
}

impl<T: ?Sized> Address for *const T {
    type Raw = usize;

    fn raw_addr(&self) -> usize {
        self.to_raw_parts().0 as usize
    }

    fn deep_map_addr(self, f: impl FnMut(usize) -> usize) -> Self {
        self.map_addr(f)
    }
}

impl<T: ?Sized> Address for *mut T {
    type Raw = usize;

    fn raw_addr(&self) -> usize {
        self.to_raw_parts().0 as usize
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
