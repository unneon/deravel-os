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
        $field_vis fn $field_name<'a, Access>(self: &'a mut Volatile<Self, Access>) -> Volatile<'a, $field_type, crate::util::volatile::$access> {
            unsafe { Volatile::new(self.0.byte_add(core::mem::offset_of!($struct, $field_name)) as *mut $field_type) }
        })*
    }
}

pub trait Readable {}
pub trait Writable {}

pub struct Volatile<'a, T, Access = ReadWrite>(*mut T, PhantomData<(&'a mut T, Access)>);

pub struct VolatileCellWithPureReads<T>(UnsafeCell<T>);

pub struct Readonly;
pub struct ReadWrite;

impl<'a, T, Access> Volatile<'a, T, Access> {
    pub unsafe fn new(pointer: *mut T) -> Volatile<'a, T, Access> {
        Volatile(pointer, PhantomData)
    }
}

impl<T: Copy, Access: Readable> Volatile<'_, T, Access> {
    pub fn read(&self) -> T {
        unsafe { self.0.read_volatile() }
    }
}

impl<T, Access: Writable> Volatile<'_, T, Access> {
    pub fn write(&mut self, value: T) {
        unsafe { self.0.write_volatile(value) }
    }
}

impl<T: BitOr<Output = T>, Access: Readable + Writable> Volatile<'_, T, Access> {
    pub fn write_bitor(&mut self, value: T) {
        unsafe { self.0.write_volatile(self.0.read_volatile() | value) }
    }
}

impl<'a, T: 'a, Access, const N: usize> Volatile<'a, [T; N], Access> {
    pub fn index(&self, index: usize) -> Volatile<'a, T, Access> {
        assert!(index < N);
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

impl<T, Access> Deref for Volatile<'_, T, Access> {
    type Target = T;

    #[track_caller]
    fn deref(&self) -> &T {
        unreachable!()
    }
}

unsafe impl<T: Send, Access> Send for Volatile<'_, T, Access> {}

unsafe impl<T: Send> Sync for Volatile<'_, T, Readonly> {}
