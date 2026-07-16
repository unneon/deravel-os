use crate::current_pid;
use core::sync::atomic::{AtomicUsize, Ordering};
use deravel_types::{
    Actor, Capability, CapabilityCertificate, CapabilityCertificateValue, Interface, RawCapability,
    get_capability_certificate_page,
};
use log::*;

pub(crate) static CAPABILITIES_ALLOCATED: AtomicUsize = AtomicUsize::new(1);

pub(crate) fn grant_unhandled<T: Interface>(grantee: impl Into<Actor>) -> Capability<T> {
    let grantee = grantee.into();
    let certificate = allocate_certificate();
    certificate.store(
        CapabilityCertificateValue::granted(grantee),
        Ordering::Relaxed,
    );
    let cap = unsafe { Capability::new(RawCapability::from_ref(certificate)) };
    let t_name = T::NAME;
    trace!("granted {cap:?} {t_name} to {:?}", grantee);
    cap
}

pub fn forward<T: Interface>(cap: Capability<T>, forwardee: impl Into<Actor>) -> Capability<T> {
    let forwardee = forwardee.into();
    let certificate = allocate_certificate();
    certificate.store(
        CapabilityCertificateValue::forwarded(forwardee, cap.as_raw()),
        Ordering::Relaxed,
    );
    let forwarded = unsafe { Capability::new(RawCapability::from_ref(certificate)) };
    let t_name = T::NAME;
    trace!("forwarded {cap:?} {t_name} as {forwarded:?} to {forwardee:?}");
    forwarded
}

fn allocate_certificate() -> &'static CapabilityCertificate {
    let index = CAPABILITIES_ALLOCATED.fetch_add(1, Ordering::Relaxed);
    assert!(
        index < 4096 / size_of::<CapabilityCertificateValue>(),
        "out of capability certificate slots"
    );
    &get_capability_certificate_page(current_pid().into())[index]
}
