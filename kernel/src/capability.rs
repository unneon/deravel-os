use crate::process::PROCESS_COUNT;
use crate::sync::Mutex;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering};
use deravel_types::*;
use log::trace;

pub trait Handler<T> {
    fn call_method(&self, method: usize, args: &[u8], sender: ProcessId) -> Vec<u8>;

    fn map_stream(&self, stream: usize) -> &'static UntypedRingBuffer;

    fn shared_memory(&self) -> (usize, usize);
}

pub trait RawHandler {
    fn call_method(&self, method: usize, args: &[u8], sender: ProcessId) -> Vec<u8>;

    fn map_stream(&self, stream: usize) -> &'static UntypedRingBuffer;

    fn shared_memory(&self) -> (usize, usize);
}

struct TypedHandler<T, H: 'static>(&'static H, PhantomData<T>);

static CAPABILITY_PAGES: [CapabilityPage; PROCESS_COUNT + 1] =
    [const { CapabilityPage([const { CapabilityCertificate::new() }; _]) }; _];

static ALLOCATED_COUNT: AtomicUsize = AtomicUsize::new(0);

static HANDLERS: [Mutex<Option<&'static (dyn RawHandler + Sync)>>;
    PAGE_SIZE / size_of::<CapabilityCertificateValue>()] = [const { Mutex::new(None) }; _];

impl<T, H: Handler<T>> RawHandler for TypedHandler<T, H> {
    fn call_method(&self, method: usize, args: &[u8], sender: ProcessId) -> Vec<u8> {
        self.0.call_method(method, args, sender)
    }

    fn map_stream(&self, stream: usize) -> &'static UntypedRingBuffer {
        self.0.map_stream(stream)
    }

    fn shared_memory(&self) -> (usize, usize) {
        self.0.shared_memory()
    }
}

pub fn grant_kernel_capability<T: 'static + Sync>(
    grantee: ProcessId,
    handler: &'static (impl Handler<T> + Sync),
) -> Capability<T> {
    let cap = reserve_kernel_capability(handler);
    // TODO: Race condition, PID 0 can use the capability.
    kernel_capability_page().0[cap.local_index()].store(
        CapabilityCertificateValue::granted(grantee),
        Ordering::Relaxed,
    );
    cap
}

pub fn reserve_kernel_capability<T: 'static + Sync>(
    handler: &'static (impl Handler<T> + Sync),
) -> Capability<T> {
    let local_index = ALLOCATED_COUNT.fetch_add(1, Ordering::Relaxed);
    *HANDLERS[local_index].lock() = Some(Box::leak(Box::new(TypedHandler(handler, PhantomData))));
    unsafe { Capability::new(RawCapability::new(Actor::Kernel, local_index)) }
}

pub fn get_handler(local_index: usize) -> &'static dyn RawHandler {
    HANDLERS[local_index].lock().unwrap()
}

pub fn capability_page(pid: ProcessId) -> &'static CapabilityPage {
    &CAPABILITY_PAGES[pid.as_u16() as usize]
}

pub fn kernel_capability_page() -> &'static CapabilityPage {
    &CAPABILITY_PAGES[0]
}

pub fn capability_pages_physical_address() -> usize {
    &CAPABILITY_PAGES as *const _ as usize
}

pub fn validate_untrusted_capability(
    farthest_cap: RawCapability,
    current_pid: ProcessId,
) -> RawCapability {
    trace!("validating capability {farthest_cap:?} presented by {current_pid:?}");
    let mut capability = farthest_cap;
    let mut sender = Actor::Userspace(current_pid);
    loop {
        let certifier = capability.certifier();
        let certificate = &match certifier {
            Actor::Userspace(pid) => capability_page(pid),
            Actor::Kernel => kernel_capability_page(),
        }
        .0[capability.local_index()];
        match certificate.load(Ordering::Relaxed).unpack() {
            CapabilityCertificateUnpacked::Granted { grantee } => {
                trace!("... granted by {certifier:?} to {grantee:?}");
                assert!(grantee == sender);
                break capability;
            }
            CapabilityCertificateUnpacked::Forwarded { forwardee, inner } => {
                trace!("... forwarded {inner:?} by {certifier:?} to {forwardee:?}");
                assert!(forwardee == sender);
                capability = inner;
                sender = certifier;
            }
        }
    }
}
