use crate::current_pid;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering};
use deravel_types::{
    Actor, Capability, CapabilityCertificate, CapabilityCertificateUnpacked,
    CapabilityCertificateValue, Interface, PAGE_SIZE, ProcessId, RawCapability,
    get_capability_certificate_page,
};
use log::trace;

pub trait Handler<T> {
    fn call_method(&mut self, method: usize, args: &[u8], sender: ProcessId) -> Vec<u8>;
}

pub trait RawHandler {
    fn call_method(&mut self, method: usize, args: &[u8], sender: ProcessId) -> Vec<u8>;
}

struct TypedHandler<T, H: 'static>(&'static mut H, PhantomData<T>);

// TODO: Unify the allocation counts.

static CAPABILITIES_ALLOCATED: AtomicUsize = AtomicUsize::new(0);

pub static mut HANDLERS: [Option<&'static mut (dyn RawHandler + Sync)>;
    PAGE_SIZE / size_of::<CapabilityCertificateValue>()] = [const { None }; _];

static HANDLERS_ALLOCATED: AtomicUsize = AtomicUsize::new(1);

impl<T, H: Handler<T>> RawHandler for TypedHandler<T, H> {
    fn call_method(&mut self, method: usize, args: &[u8], sender: ProcessId) -> Vec<u8> {
        self.0.call_method(method, args, sender)
    }
}

pub fn grant_capability2<T: 'static + Interface + Sync>(
    grantee: impl Into<Actor>,
    handler: &'static mut (impl Handler<T> + Sync),
) -> Capability<T> {
    let grantee = grantee.into();
    let local_index = HANDLERS_ALLOCATED.fetch_add(1, Ordering::Relaxed);
    unsafe { HANDLERS[local_index] = Some(Box::leak(Box::new(TypedHandler(handler, PhantomData)))) }
    let certificate = allocate_certificate();
    certificate.store(
        CapabilityCertificateValue::granted(grantee),
        Ordering::Relaxed,
    );
    let cap = Capability(RawCapability::from_pointer(certificate), PhantomData);
    let t_name = T::NAME;
    trace!("granted {cap:?} {t_name} to {grantee:?}");
    cap
}

pub fn register_root_capability<T: 'static + Sync>(handler: &'static mut (impl Handler<T> + Sync)) {
    unsafe { HANDLERS[0] = Some(Box::leak(Box::new(TypedHandler(handler, PhantomData)))) }
}

pub fn forward_capability<T: Interface, U: Interface>(
    cap: Capability<T>,
    forwardee: Capability<U>,
) -> Capability<T> {
    let certificate = allocate_certificate();
    certificate.store(
        CapabilityCertificateValue::forwarded(forwardee.certifier(), cap.0),
        Ordering::Relaxed,
    );
    let forwarded = Capability(RawCapability::from_pointer(certificate), PhantomData);
    let t_name = T::NAME;
    let u_name = U::NAME;
    trace!("forwarded {cap:?} {t_name} as {forwarded:?} to {forwardee:?} {u_name}");
    forwarded
}

pub fn forward_capability_by_pid<T: Interface>(
    cap: Capability<T>,
    forwardee: ProcessId,
) -> Capability<T> {
    let certificate = allocate_certificate();
    certificate.store(
        CapabilityCertificateValue::forwarded(forwardee.into(), cap.0),
        Ordering::Relaxed,
    );
    let forwarded = Capability(RawCapability::from_pointer(certificate), PhantomData);
    let t_name = T::NAME;
    trace!("forwarded {cap:?} {t_name} as {forwarded:?} to {forwardee:?}");
    forwarded
}

pub fn validate_capability(cap: RawCapability, claimer: Actor) -> RawCapability {
    trace!("validating capability {cap:?} from process {claimer:?}");
    let mut capability = cap;
    let mut sender = claimer;
    let original = loop {
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
    assert!(original.certifier() == current_pid().into());
    original
}

fn read_certificate(cap: RawCapability) -> CapabilityCertificateValue {
    get_capability_certificate_page(cap.certifier())[cap.local_index()].load(Ordering::Relaxed)
}

fn allocate_certificate() -> &'static CapabilityCertificate {
    let index = CAPABILITIES_ALLOCATED.fetch_add(1, Ordering::Relaxed) + 1;
    assert!(
        index < 4096 / size_of::<CapabilityCertificateValue>(),
        "out of capability certificate slots"
    );
    &get_capability_certificate_page(current_pid().into())[index]
}
