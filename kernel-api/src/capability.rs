use crate::current_pid;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering};
use deravel_types::ProcessId;
use deravel_types::capability::{
    CAPABILITIES_START, Capability, CapabilityCertificate, CapabilityCertificateUnpacked,
};
use log::trace;
use serde::{Deserialize, Deserializer};

#[derive(Clone, Copy)]
pub struct CallableCapability<T>(pub *const CapabilityCertificate, pub PhantomData<T>);

static CAPABILITIES_ALLOCATED: AtomicUsize = AtomicUsize::new(0);

impl<T> From<CallableCapability<T>> for Capability {
    fn from(capability: CallableCapability<T>) -> Self {
        unsafe { core::mem::transmute(capability) }
    }
}

impl<T> core::ops::Deref for CallableCapability<T> {
    type Target = Capability;

    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute::<&CallableCapability<_>, &Capability>(&self) }
    }
}

impl<T> core::fmt::Debug for CallableCapability<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0 as usize)
    }
}

impl<'de, T> Deserialize<'de> for CallableCapability<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(CallableCapability(
            usize::deserialize(deserializer)? as *const CapabilityCertificate,
            PhantomData,
        ))
    }
}

pub fn grant_capability(grantee: ProcessId) -> Capability {
    let certificate = allocate_certificate();
    *certificate = CapabilityCertificate::granted(grantee);
    let cap = Capability(certificate);
    trace!("granted {cap:?} to {grantee:?}");
    cap
}

pub fn forward_capability(cap: Capability, forwardee: Capability) -> Capability {
    let certificate = allocate_certificate();
    *certificate = CapabilityCertificate::forwarded(forwardee.certifier(), cap.into());
    let forwarded = Capability(certificate);
    trace!("forwarded {cap:?} as {forwarded:?} to {forwardee:?}");
    forwarded
}

pub fn validate_capability(cap: Capability, claimer: ProcessId) -> Capability {
    trace!("validating capability {cap:?} from process {claimer:?}");
    let mut capability = cap;
    let mut sender = claimer;
    let original = loop {
        assert!(capability.is_pointer_valid());
        let certifier = capability.certifier();
        match read_certificate(capability).unpack() {
            CapabilityCertificateUnpacked::Granted { grantee } => {
                trace!("... granted from {certifier:?} to {grantee:?}");
                assert!(grantee == sender);
                break capability;
            }
            CapabilityCertificateUnpacked::Forwarded { forwardee, inner } => {
                trace!("... forwarded {inner:?} from {certifier:?} to {forwardee:?}");
                assert!(forwardee == sender);
                capability = inner;
                sender = certifier;
            }
        }
    };
    assert!(original.certifier() == current_pid());
    original
}

fn read_certificate(cap: Capability) -> CapabilityCertificate {
    assert!(cap.is_pointer_valid());
    unsafe { *cap.0 }
}

fn allocate_certificate() -> &'static mut CapabilityCertificate {
    let index = CAPABILITIES_ALLOCATED.fetch_add(1, Ordering::Relaxed);
    assert!(
        index < 4096 / size_of::<CapabilityCertificate>(),
        "out of capability certificate slots"
    );
    let all_certificates = CAPABILITIES_START as *mut CapabilityCertificate;
    let our_certificates = unsafe { all_certificates.byte_add(4096 * current_pid().0) };
    let certificate = unsafe { our_certificates.add(index) };
    unsafe { &mut *certificate }
}
