use crate::drvli::{Handler, RawHandler};
use crate::process::PROCESS_COUNT;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering};
use deravel_types::{Actor, Capability, CapabilityCertificate, PAGE_SIZE, RawCapability};

#[repr(C, align(4096))]
#[derive(Clone, Copy)]
pub struct CapabilityPage(
    pub [CapabilityCertificate; PAGE_SIZE / size_of::<CapabilityCertificate>()],
);

struct TypedHandler<T, H: 'static>(&'static H, PhantomData<T>);

pub static mut CAPABILITY_PAGES: [CapabilityPage; PROCESS_COUNT + 1] = [CapabilityPage::empty(); _];
pub static mut HANDLERS: [Option<Box<dyn RawHandler>>;
    PAGE_SIZE / size_of::<CapabilityCertificate>()] = [const { None }; _];

static ALLOCATED_COUNT: AtomicUsize = AtomicUsize::new(0);

impl CapabilityPage {
    const fn empty() -> CapabilityPage {
        CapabilityPage([CapabilityCertificate::empty(); _])
    }
}

impl<T, H: Handler<T>> RawHandler for TypedHandler<T, H> {
    fn handle(&self, method: usize, args: &[u8]) -> Vec<u8> {
        self.0.handle(method, args)
    }
}

pub fn reserve_kernel_capability<T: 'static>(handler: &'static impl Handler<T>) -> Capability<T> {
    let local_index = ALLOCATED_COUNT.fetch_add(1, Ordering::Relaxed);
    unsafe { HANDLERS[local_index] = Some(Box::new(TypedHandler(handler, PhantomData))) }
    Capability(RawCapability::new(Actor::Kernel, local_index), PhantomData)
}
