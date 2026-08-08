use crate::{Actor, ProcessId, RawCapability};
use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
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

impl CapabilityCertificate {
    pub const fn new() -> CapabilityCertificate {
        CapabilityCertificate(AtomicU64::new(0))
    }

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
                Actor::Kernel => 0,
            },
            payload: 0,
        }
    }

    pub fn forwarded(
        forwardee: impl Into<Actor>,
        capability: RawCapability,
    ) -> CapabilityCertificateValue {
        CapabilityCertificateValue {
            grantee: match forwardee.into() {
                Actor::Userspace(pid) => pid.as_u16() as u32,
                Actor::Kernel => 0,
            },
            payload: capability.as_usize() as u32,
        }
    }

    pub fn unpack(self) -> CapabilityCertificateUnpacked {
        let recipient = if self.grantee == 0 {
            Actor::Kernel
        } else {
            Actor::Userspace(ProcessId::new(self.grantee as u16))
        };
        if self.payload == 0 {
            CapabilityCertificateUnpacked::Granted { grantee: recipient }
        } else {
            CapabilityCertificateUnpacked::Forwarded {
                forwardee: recipient,
                inner: RawCapability::try_from(self.payload as *const CapabilityCertificate)
                    .unwrap(),
            }
        }
    }

    pub fn replace_recipient(self, recipient: impl Into<Actor>) -> CapabilityCertificateValue {
        match self.unpack() {
            CapabilityCertificateUnpacked::Granted { .. } => Self::granted(recipient),
            CapabilityCertificateUnpacked::Forwarded { inner, .. } => {
                Self::forwarded(recipient, inner)
            }
        }
    }
}
