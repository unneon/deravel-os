use crate::process::PROCESS_COUNT;
use crate::sync::Mutex;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering};
use deravel_types::{
    Actor, Capability, CapabilityCertificateValue, CapabilityPage, PAGE_SIZE, RawCapability,
};

pub trait Handler<T> {
    fn handle(&self, method: usize, args: &[u8]) -> Vec<u8>;
}

pub trait RawHandler {
    fn handle(&self, method: usize, args: &[u8]) -> Vec<u8>;
}

struct TypedHandler<T, H: 'static>(&'static H, PhantomData<T>);

pub static CAPABILITY_PAGES: [CapabilityPage; PROCESS_COUNT + 1] = unsafe { core::mem::zeroed() };

static ALLOCATED_COUNT: AtomicUsize = AtomicUsize::new(0);

static HANDLERS: [Mutex<Option<&'static (dyn RawHandler + Sync)>>;
    PAGE_SIZE / size_of::<CapabilityCertificateValue>()] = [const { Mutex::new(None) }; _];

impl<T, H: Handler<T>> RawHandler for TypedHandler<T, H> {
    fn handle(&self, method: usize, args: &[u8]) -> Vec<u8> {
        self.0.handle(method, args)
    }
}

pub fn reserve_kernel_capability<T: 'static + Sync>(
    handler: &'static (impl Handler<T> + Sync),
) -> Capability<T> {
    let local_index = ALLOCATED_COUNT.fetch_add(1, Ordering::Relaxed);
    *HANDLERS[local_index].lock() = Some(Box::leak(Box::new(TypedHandler(handler, PhantomData))));
    Capability(RawCapability::new(Actor::Kernel, local_index), PhantomData)
}

pub fn get_handler(local_index: usize) -> &'static dyn RawHandler {
    HANDLERS[local_index].lock().unwrap()
}
