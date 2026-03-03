use crate::ProcessId;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy)]
pub struct Capability(pub *const CapabilityCertificate);

#[derive(Clone, Copy)]
pub struct CapabilityCertificate(usize);

#[derive(Debug)]
pub enum CapabilityCertificateUnpacked {
    Granted {
        grantee: ProcessId,
    },
    Forwarded {
        forwardee: ProcessId,
        inner: Capability,
    },
}

pub const CAPABILITIES_START: usize = 0x2000000;
pub const CAPABILITIES_END: usize = 0x3000000;

impl Capability {
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

    pub fn forwarded(forwardee: ProcessId, capability: Capability) -> CapabilityCertificate {
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
                inner: Capability(raw_inner as *const CapabilityCertificate),
            }
        }
    }
}

impl core::fmt::Debug for Capability {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0 as usize)
    }
}

impl<'de> Deserialize<'de> for Capability {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Capability(
            usize::deserialize(deserializer)? as *const CapabilityCertificate
        ))
    }
}

impl Serialize for Capability {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.0 as usize).serialize(serializer)
    }
}
