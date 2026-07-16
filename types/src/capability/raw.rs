use crate::capability::pages::{
    CAPABILITIES_END, CAPABILITIES_START, get_capability_certificate_page,
};
use crate::{Actor, CapabilityCertificate, CapabilityCertificateUnpacked, PAGE_SIZE, ProcessId};
use core::sync::atomic::Ordering;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub struct CapabilityError(RawCapability, ProcessId);

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct RawCapability(&'static CapabilityCertificate);

#[derive(Debug)]
pub struct InvalidCapabilityError;

impl RawCapability {
    pub fn new(certifier: impl Into<Actor>, local_index: usize) -> RawCapability {
        assert!(local_index < PAGE_SIZE / size_of::<CapabilityCertificate>());
        RawCapability(&get_capability_certificate_page(certifier.into())[local_index])
    }

    pub fn from_ref(ptr: &'static CapabilityCertificate) -> RawCapability {
        assert!(is_ptr_valid(ptr));
        RawCapability(ptr)
    }

    pub fn certifier(self) -> Actor {
        let page_index = (self.as_usize() - CAPABILITIES_START) / PAGE_SIZE;
        if page_index == 0 {
            Actor::Kernel
        } else {
            Actor::Userspace(ProcessId::new(page_index as u16))
        }
    }

    pub fn local_index(self) -> usize {
        (self.as_usize() % PAGE_SIZE) / 8
    }

    pub fn as_usize(self) -> usize {
        self.0 as *const CapabilityCertificate as usize
    }

    pub fn validate(self, orig_claimer: ProcessId) -> Result<RawCapability, CapabilityError> {
        let mut capability = self;
        let mut claimer = Actor::Userspace(orig_claimer);
        loop {
            let certifier = capability.certifier();
            let certificate = &get_capability_certificate_page(certifier)[capability.local_index()];
            match certificate.load(Ordering::Relaxed).unpack() {
                CapabilityCertificateUnpacked::Granted { grantee } if grantee == claimer => {
                    break Ok(capability);
                }
                CapabilityCertificateUnpacked::Forwarded { forwardee, inner }
                    if forwardee == claimer =>
                {
                    capability = inner;
                    claimer = certifier;
                }
                _ => return Err(CapabilityError(self, orig_claimer)),
            }
        }
    }
}

impl core::fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "[{:?}] claimed {:?}", self.1, self.0)?;
        let mut capability = self.0;
        let mut claimer = Actor::Userspace(self.1);
        loop {
            let certifier = capability.certifier();
            let certificate = &get_capability_certificate_page(certifier)[capability.local_index()];
            match certificate.load(Ordering::Relaxed).unpack() {
                CapabilityCertificateUnpacked::Granted { grantee } => {
                    return write!(f, " actually granted to {grantee:?}");
                }
                CapabilityCertificateUnpacked::Forwarded { forwardee, inner } => {
                    if forwardee != claimer {
                        return write!(f, " actually forwarded to {forwardee:?}");
                    } else {
                        write!(f, " forwarded by {:?} from {:?}", inner.certifier(), inner)?;
                        capability = inner;
                        claimer = certifier;
                    }
                }
            }
        }
    }
}

impl core::fmt::Debug for CapabilityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        <CapabilityError as core::fmt::Display>::fmt(self, f)
    }
}

impl TryFrom<*const CapabilityCertificate> for RawCapability {
    type Error = InvalidCapabilityError;

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn try_from(ptr: *const CapabilityCertificate) -> Result<Self, Self::Error> {
        if !is_ptr_valid(ptr) {
            return Err(InvalidCapabilityError);
        }
        Ok(RawCapability(unsafe { &*ptr }))
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
        Ok(RawCapability::try_from(cap).unwrap())
    }
}

impl Serialize for RawCapability {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_usize().serialize(serializer)
    }
}

impl core::fmt::Display for InvalidCapabilityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid capability")
    }
}

fn is_ptr_valid(maybe_cap: *const CapabilityCertificate) -> bool {
    let in_range = (CAPABILITIES_START..CAPABILITIES_END).contains(&(maybe_cap as usize));
    let aligned = maybe_cap.is_aligned();
    in_range && aligned
}
