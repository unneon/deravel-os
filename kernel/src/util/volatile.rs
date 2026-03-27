use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::{BitOr, Deref};

pub macro volatile_struct($struct_vis:vis $struct:ident $(<$($param:ident),*>)? $(where $param0:ident: $req0:ident)? $($field_vis:vis $field_name:ident: $access:ident $field_type:ty,)*) {
    #[repr(C)]
    $struct_vis struct $struct $(<$($param),*>)? {
        $($field_vis $field_name: $field_type,)*
    }

    impl$(<$($param),*>)? $struct $(<$($param),*>)? $(where $param0: $req0)? {
        $(#[allow(dead_code)]
        $field_vis fn $field_name(self: Volatile<Self>) -> Volatile<$field_type, crate::util::volatile::$access> {
            unsafe { Volatile::new(self.0.byte_add(core::mem::offset_of!($struct, $field_name)) as *mut $field_type) }
        })*
    }
}

pub trait Readable {}
pub trait Writable {}

pub struct Volatile<T, Access = ReadWrite>(*mut T, PhantomData<Access>);

pub struct VolatileCellWithPureReads<T>(UnsafeCell<T>);

pub struct Readonly;
pub struct ReadWrite;

impl<T, Access> Volatile<T, Access> {
    pub unsafe fn new(pointer: *mut T) -> Volatile<T, Access> {
        Volatile(pointer, PhantomData)
    }
}

impl<T: Copy, Access: Readable> Volatile<T, Access> {
    pub fn read(&self) -> T {
        unsafe { self.0.read_volatile() }
    }
}

impl<T, Access: Writable> Volatile<T, Access> {
    pub fn write(&self, value: T) {
        unsafe { self.0.write_volatile(value) }
    }
}

impl<T: BitOr<Output = T>, Access: Readable + Writable> Volatile<T, Access> {
    pub fn write_bitor(&self, value: T) {
        unsafe { self.0.write_volatile(self.0.read_volatile() | value) }
    }
}

impl<T, Access, const N: usize> Volatile<[T; N], Access> {
    pub fn index(&self, index: usize) -> Volatile<T, Access> {
        unsafe { Volatile::new((self.0 as *mut T).add(index)) }
    }
}

impl<T: Copy> VolatileCellWithPureReads<T> {
    pub fn read(&self) -> T {
        unsafe { self.0.get().read_volatile() }
    }

    pub fn write(&mut self, value: T) {
        unsafe { self.0.get().write_volatile(value) }
    }

    pub fn write_bitor(&mut self, value: T)
    where
        T: BitOr<Output = T>,
    {
        let left = unsafe { self.0.get().read_volatile() };
        unsafe { self.0.get().write_volatile(left | value) }
    }
}

impl Readable for Readonly {}

impl Readable for ReadWrite {}

impl Writable for ReadWrite {}

impl<T, Access> From<Volatile<T, Access>> for *mut T {
    fn from(value: Volatile<T, Access>) -> Self {
        value.0
    }
}

impl<T, Access> Deref for Volatile<T, Access> {
    type Target = T;

    #[track_caller]
    fn deref(&self) -> &T {
        unreachable!()
    }
}

impl<T, Access> Clone for Volatile<T, Access> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, Access> Copy for Volatile<T, Access> {}

impl<T, Access> core::fmt::Display for Volatile<T, Access> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0 as usize)
    }
}

unsafe impl<T, Access> Send for Volatile<T, Access> {}

unsafe impl<T, Access> Sync for Volatile<T, Access> {}
