use crate::{PAGE_SIZE, ProcessId};
use core::marker::PhantomData;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Deserialize, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Capability<T>(pub RawCapability, pub PhantomData<T>);

#[derive(Clone, Copy)]
pub struct CapabilityCertificate {
    grantee: u32,
    payload: u32,
}

#[derive(Debug)]
pub enum CapabilityCertificateUnpacked {
    Granted {
        grantee: Actor,
    },
    Forwarded {
        forwardee: Actor,
        inner: RawCapability,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Actor {
    Userspace(ProcessId),
    Kernel,
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct RawCapability(*const CapabilityCertificate);

pub const CAPABILITIES_START: usize = 0x2000000;
pub const CAPABILITIES_END: usize = 0x3000000;
pub const MAX_PROCESSES: usize = (CAPABILITIES_END - CAPABILITIES_START) / PAGE_SIZE - 1;

impl RawCapability {
    pub fn new(certifier: impl Into<Actor>, local_index: usize) -> RawCapability {
        assert!(local_index < PAGE_SIZE / size_of::<CapabilityCertificate>());
        let pointer = match certifier.into() {
            Actor::Userspace(pid) => {
                CAPABILITIES_START
                    + pid.0 * PAGE_SIZE
                    + local_index * size_of::<CapabilityCertificate>()
            }
            Actor::Kernel => {
                CAPABILITIES_END - PAGE_SIZE + local_index * size_of::<CapabilityCertificate>()
            }
        };
        RawCapability(pointer as *const _)
    }

    pub fn from_pointer(pointer: *mut CapabilityCertificate) -> RawCapability {
        assert!(is_capability_pointer_valid(pointer));
        RawCapability(pointer)
    }

    pub fn certifier(self) -> Actor {
        let page_index = (self.0 as usize - CAPABILITIES_START) / PAGE_SIZE;
        if page_index < MAX_PROCESSES {
            Actor::Userspace(ProcessId(page_index))
        } else {
            Actor::Kernel
        }
    }

    pub fn local_index(self) -> usize {
        (self.0 as usize % PAGE_SIZE) / 8
    }

    pub fn as_usize(self) -> usize {
        self.0 as _
    }
}

impl CapabilityCertificate {
    pub const fn empty() -> CapabilityCertificate {
        CapabilityCertificate {
            grantee: u32::MAX,
            payload: 0,
        }
    }

    pub fn granted(grantee: impl Into<Actor>) -> CapabilityCertificate {
        CapabilityCertificate {
            grantee: match grantee.into() {
                Actor::Userspace(pid) => pid.0 as u32,
                Actor::Kernel => MAX_PROCESSES as u32,
            },
            payload: 0,
        }
    }

    pub fn forwarded(forwardee: Actor, capability: RawCapability) -> CapabilityCertificate {
        CapabilityCertificate {
            grantee: match forwardee {
                Actor::Userspace(pid) => pid.0 as u32,
                Actor::Kernel => MAX_PROCESSES as u32,
            },
            payload: capability.0 as u32,
        }
    }

    pub fn unpack(self) -> CapabilityCertificateUnpacked {
        let grantee_or_forwardee = if (self.grantee as usize) < MAX_PROCESSES {
            Actor::Userspace(ProcessId(self.grantee as usize))
        } else {
            Actor::Kernel
        };
        if self.payload == 0 {
            CapabilityCertificateUnpacked::Granted {
                grantee: grantee_or_forwardee,
            }
        } else {
            let inner = self.payload as *const CapabilityCertificate;
            assert!(is_capability_pointer_valid(inner));
            CapabilityCertificateUnpacked::Forwarded {
                forwardee: grantee_or_forwardee,
                inner: RawCapability(inner),
            }
        }
    }
}

impl From<ProcessId> for Actor {
    fn from(value: ProcessId) -> Self {
        Actor::Userspace(value)
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

impl<'de> Deserialize<'de> for RawCapability {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let cap = usize::deserialize(deserializer)? as *const CapabilityCertificate;
        assert!(is_capability_pointer_valid(cap));
        Ok(RawCapability(cap))
    }
}

impl Serialize for RawCapability {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.0 as usize).serialize(serializer)
    }
}

pub fn get_capability_certificate_page(actor: Actor) -> *mut CapabilityCertificate {
    let offset = match actor {
        Actor::Userspace(pid) => pid.0,
        Actor::Kernel => MAX_PROCESSES,
    };
    (CAPABILITIES_START + PAGE_SIZE * offset) as _
}

fn is_capability_pointer_valid(maybe_cap: *const CapabilityCertificate) -> bool {
    let in_range = (CAPABILITIES_START..CAPABILITIES_END).contains(&(maybe_cap as usize));
    let aligned = maybe_cap.is_aligned();
    in_range && aligned
}
