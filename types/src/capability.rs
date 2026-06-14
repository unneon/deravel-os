use crate::{PAGE_SIZE, ProcessId};
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU64, Ordering};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Deserialize, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Capability<T>(pub RawCapability, pub PhantomData<T>);

pub struct CapabilityCertificate(AtomicU64);

#[derive(Clone, Copy)]
pub struct CapabilityCertificateValue {
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

#[repr(align(4096))]
pub struct CapabilityPage(pub [CapabilityCertificate; CAPABILITIES_PER_PAGE]);

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Actor {
    Userspace(ProcessId),
    Kernel,
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct RawCapability(&'static CapabilityCertificate);

pub const CAPABILITIES_PER_PAGE: usize = PAGE_SIZE / size_of::<CapabilityCertificate>();

pub const CAPABILITIES_START: usize = 0x2000000;
pub const CAPABILITIES_END: usize = 0x3000000;

pub const MAX_PROCESSES: usize = (CAPABILITIES_END - CAPABILITIES_START) / PAGE_SIZE - 1;

impl RawCapability {
    pub fn new(certifier: impl Into<Actor>, local_index: usize) -> RawCapability {
        assert!(local_index < PAGE_SIZE / size_of::<CapabilityCertificate>());
        RawCapability(&get_capability_certificate_page(certifier.into())[local_index])
    }

    pub fn from_ref(pointer: &'static CapabilityCertificate) -> RawCapability {
        assert!(is_capability_pointer_valid(pointer));
        RawCapability(pointer)
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn from_ptr(pointer: *const CapabilityCertificate) -> RawCapability {
        assert!(is_capability_pointer_valid(pointer));
        RawCapability(unsafe { &*pointer })
    }

    pub fn certifier(self) -> Actor {
        let page_index = (self.as_usize() - CAPABILITIES_START) / PAGE_SIZE;
        if page_index < MAX_PROCESSES {
            Actor::Userspace(ProcessId::new(page_index as u16))
        } else {
            Actor::Kernel
        }
    }

    pub fn local_index(self) -> usize {
        (self.as_usize() % PAGE_SIZE) / 8
    }

    pub fn as_usize(self) -> usize {
        self.0 as *const CapabilityCertificate as usize
    }
}

impl CapabilityCertificate {
    pub fn load(&self, ordering: Ordering) -> CapabilityCertificateValue {
        unsafe { core::mem::transmute::<u64, CapabilityCertificateValue>(self.0.load(ordering)) }
    }

    pub fn store(&self, value: CapabilityCertificateValue, ordering: Ordering) {
        self.0.store(
            unsafe { core::mem::transmute::<CapabilityCertificateValue, u64>(value) },
            ordering,
        )
    }
}

impl CapabilityCertificateValue {
    pub const fn empty() -> CapabilityCertificateValue {
        CapabilityCertificateValue {
            grantee: u32::MAX,
            payload: 0,
        }
    }

    pub fn granted(grantee: impl Into<Actor>) -> CapabilityCertificateValue {
        CapabilityCertificateValue {
            grantee: match grantee.into() {
                Actor::Userspace(pid) => pid.as_u16() as u32,
                Actor::Kernel => MAX_PROCESSES as u32,
            },
            payload: 0,
        }
    }

    pub fn forwarded(forwardee: Actor, capability: RawCapability) -> CapabilityCertificateValue {
        CapabilityCertificateValue {
            grantee: match forwardee {
                Actor::Userspace(pid) => pid.as_u16() as u32,
                Actor::Kernel => MAX_PROCESSES as u32,
            },
            payload: capability.as_usize() as u32,
        }
    }

    pub fn unpack(self) -> CapabilityCertificateUnpacked {
        let grantee_or_forwardee = if (self.grantee as usize) < MAX_PROCESSES {
            Actor::Userspace(ProcessId::new(self.grantee as u16))
        } else {
            Actor::Kernel
        };
        if self.payload == 0 {
            CapabilityCertificateUnpacked::Granted {
                grantee: grantee_or_forwardee,
            }
        } else {
            CapabilityCertificateUnpacked::Forwarded {
                forwardee: grantee_or_forwardee,
                inner: RawCapability::from_ptr(self.payload as *const CapabilityCertificate),
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

impl core::fmt::Debug for Actor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Actor::Userspace(pid) => write!(f, "{}", pid.as_u16()),
            Actor::Kernel => write!(f, "kernel"),
        }
    }
}

impl<T> core::fmt::Debug for Capability<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.0, f)
    }
}

impl core::fmt::Debug for RawCapability {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.as_usize())
    }
}

impl<'de> Deserialize<'de> for RawCapability {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let cap = usize::deserialize(deserializer)? as *const CapabilityCertificate;
        Ok(RawCapability::from_ptr(cap))
    }
}

impl Serialize for RawCapability {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_usize().serialize(serializer)
    }
}

unsafe impl Send for RawCapability {}

unsafe impl Sync for RawCapability {}

pub fn get_capability_certificate_page(
    actor: Actor,
) -> &'static [CapabilityCertificate; CAPABILITIES_PER_PAGE] {
    let offset = match actor {
        Actor::Userspace(pid) => pid.as_u16() as usize,
        Actor::Kernel => MAX_PROCESSES,
    };
    &get_capability_certificate_pages()[offset]
}

fn get_capability_certificate_pages()
-> &'static [[CapabilityCertificate; CAPABILITIES_PER_PAGE]; MAX_PROCESSES + 1] {
    unsafe { &*(CAPABILITIES_START as *const _) }
}

fn is_capability_pointer_valid(maybe_cap: *const CapabilityCertificate) -> bool {
    let in_range = (CAPABILITIES_START..CAPABILITIES_END).contains(&(maybe_cap as usize));
    let aligned = maybe_cap.is_aligned();
    in_range && aligned
}
