use crate::RawCapability;
use core::marker::PhantomData;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Capability<T>(RawCapability, PhantomData<T>);

impl<T> Capability<T> {
    pub unsafe fn new(raw: RawCapability) -> Capability<T> {
        Capability(raw, PhantomData)
    }

    pub fn as_raw(self) -> RawCapability {
        self.0
    }
}

impl<T> Clone for Capability<T> {
    fn clone(&self) -> Capability<T> {
        *self
    }
}

impl<T> Copy for Capability<T> {}

impl<T> core::ops::Deref for Capability<T> {
    type Target = RawCapability;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> core::fmt::Debug for Capability<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.0, f)
    }
}
