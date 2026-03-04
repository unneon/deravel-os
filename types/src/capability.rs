use crate::ProcessId;
use core::marker::PhantomData;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Capability<T>(pub RawCapability, pub PhantomData<T>);

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct CapabilityCertificate(pub usize);

#[derive(Debug)]
pub enum CapabilityCertificateUnpacked {
    Granted {
        grantee: ProcessId,
    },
    Forwarded {
        forwardee: ProcessId,
        inner: RawCapability,
    },
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct RawCapability(pub *const CapabilityCertificate);

pub const CAPABILITIES_START: usize = 0x2000000;
pub const CAPABILITIES_END: usize = 0x3000000;

impl RawCapability {
    pub fn new(certifier: ProcessId, local_index: usize) -> RawCapability {
        assert!(certifier.0 < (CAPABILITIES_END - CAPABILITIES_START) / 4096);
        assert!(local_index < 4096 / size_of::<CapabilityCertificate>());
        RawCapability(
            (CAPABILITIES_START
                + certifier.0 * 4096
                + local_index * size_of::<CapabilityCertificate>()) as *const _,
        )
    }

    pub fn certifier(self) -> ProcessId {
        assert!(self.is_pointer_valid());
        ProcessId((self.0 as usize - CAPABILITIES_START) / 4096)
    }

    pub fn local_index(self) -> usize {
        assert!(self.is_pointer_valid());
        (self.0 as usize % 4096) / 8
    }

    pub fn is_pointer_valid(self) -> bool {
        let in_range = (CAPABILITIES_START..CAPABILITIES_END).contains(&(self.0 as usize));
        let aligned = self.0.is_aligned();
        in_range && aligned
    }
}

impl CapabilityCertificate {
    pub fn granted(grantee: ProcessId) -> CapabilityCertificate {
        assert!(grantee.0 < 8);
        CapabilityCertificate(grantee.0)
    }

    pub fn forwarded(forwardee: ProcessId, capability: RawCapability) -> CapabilityCertificate {
        assert!(forwardee.0 < 8);
        assert!(capability.0.is_aligned_to(8));
        CapabilityCertificate(forwardee.0 | capability.0 as usize)
    }

    pub fn unpack(self) -> CapabilityCertificateUnpacked {
        let certified = ProcessId(self.0 % 8);
        let raw_inner = self.0 ^ certified.0;
        if raw_inner == 0 {
            CapabilityCertificateUnpacked::Granted { grantee: certified }
        } else {
            CapabilityCertificateUnpacked::Forwarded {
                forwardee: certified,
                inner: RawCapability(raw_inner as *const CapabilityCertificate),
            }
        }
    }
}

impl<T> From<Capability<T>> for RawCapability {
    fn from(cap: Capability<T>) -> Self {
        cap.0
    }
}

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

impl core::fmt::Debug for RawCapability {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0 as usize)
    }
}

impl<'de, T> Deserialize<'de> for Capability<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Deserialize::deserialize(deserializer).map(|cap| Self(cap, PhantomData))
    }
}

impl<'de> Deserialize<'de> for RawCapability {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(RawCapability(
            usize::deserialize(deserializer)? as *const CapabilityCertificate
        ))
    }
}

impl<T> Serialize for Capability<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Serialize::serialize(&self.0, serializer)
    }
}

impl Serialize for RawCapability {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.0 as usize).serialize(serializer)
    }
}
