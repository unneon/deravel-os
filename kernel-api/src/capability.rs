use crate::current_pid;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering};
use deravel_types::{
    CAPABILITIES_START, Capability, CapabilityCertificate, CapabilityCertificateUnpacked,
    ProcessId, RawCapability,
};
use log::trace;

static CAPABILITIES_ALLOCATED: AtomicUsize = AtomicUsize::new(0);

pub fn grant_capability<T>(grantee: ProcessId) -> Capability<T> {
    let certificate = allocate_certificate();
    *certificate = CapabilityCertificate::granted(grantee);
    let cap = Capability(RawCapability(certificate), PhantomData);
    trace!("granted {cap:?} to {grantee:?}");
    cap
}

pub fn forward_capability<T, U>(cap: Capability<T>, forwardee: Capability<U>) -> Capability<T> {
    let cap = unsafe { core::mem::transmute::<Capability<T>, RawCapability>(cap) };
    let certificate = allocate_certificate();
    *certificate = CapabilityCertificate::forwarded(forwardee.certifier(), cap);
    let forwarded = Capability(RawCapability(certificate), PhantomData);
    trace!("forwarded {cap:?} as {forwarded:?} to {forwardee:?}");
    forwarded
}

pub fn validate_capability(cap: RawCapability, claimer: ProcessId) -> RawCapability {
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

fn read_certificate(cap: RawCapability) -> CapabilityCertificate {
    assert!(cap.is_pointer_valid());
    unsafe { *cap.0 }
}

fn allocate_certificate() -> &'static mut CapabilityCertificate {
    let index = CAPABILITIES_ALLOCATED.fetch_add(1, Ordering::Relaxed) + 1;
    assert!(
        index < 4096 / size_of::<CapabilityCertificate>(),
        "out of capability certificate slots"
    );
    let all_certificates = CAPABILITIES_START as *mut CapabilityCertificate;
    let our_certificates = unsafe { all_certificates.byte_add(4096 * current_pid().0) };
    let certificate = unsafe { our_certificates.add(index) };
    unsafe { &mut *certificate }
}
