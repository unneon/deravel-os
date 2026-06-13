use crate::current_pid;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering};
use deravel_types::{
    Actor, Capability, CapabilityCertificate, CapabilityCertificateUnpacked,
    CapabilityCertificateValue, Interface, ProcessId, RawCapability, RingBuffer,
    get_capability_certificate_page,
};
use log::*;

pub trait Handler<T, O: Copy> {
    fn call_method(
        &mut self,
        ctx: &mut Ctx<Self>,
        method: usize,
        args: &[u8],
        object: O,
        sender: ProcessId,
    ) -> Vec<u8>;
}

pub trait Observer<T, O: Copy> {
    fn observe(&mut self, ctx: OCtx<Self>, value: T, object: O);
}

pub trait RawHandler<S: ?Sized> {
    fn call_method(
        &mut self,
        server: &mut S,
        method: usize,
        args: &[u8],
        sender: ProcessId,
    ) -> (Vec<u8>, Vec<HandlerEntry<S>>);
}

pub trait RawObserver<S: ?Sized> {
    fn observe(&mut self, server: &mut S) -> Vec<HandlerEntry<S>>;
}

pub struct Ctx<'a, S: ?Sized> {
    sender: ProcessId,
    new_handlers: &'a mut Vec<HandlerEntry<S>>,
}

pub struct OCtx<'a, S: ?Sized> {
    new_handlers: &'a mut Vec<HandlerEntry<S>>,
}

pub struct Dispatch<S> {
    pub server: S,
    handlers: Vec<Option<Box<dyn RawHandler<S>>>>,
    observers: Vec<Box<dyn RawObserver<S>>>,
}

pub struct HandlerEntry<S: ?Sized> {
    local_index: usize,
    handler: Box<dyn RawHandler<S>>,
}

struct TypedHandler<T, O>(O, PhantomData<T>);

struct TypedObserver<T: 'static, O>(O, &'static RingBuffer<T>);

impl<S: ?Sized + Handler<T, O>, T, O: Copy> RawHandler<S> for TypedHandler<T, O> {
    fn call_method(
        &mut self,
        server: &mut S,
        method: usize,
        args: &[u8],
        sender: ProcessId,
    ) -> (Vec<u8>, Vec<HandlerEntry<S>>) {
        let mut new_handlers = Vec::new();
        let mut ctx = Ctx {
            sender,
            new_handlers: &mut new_handlers,
        };
        let result = server.call_method(&mut ctx, method, args, self.0, sender);
        (result, new_handlers)
    }
}

impl<S: ?Sized + Observer<T, O>, T: Copy, O: Copy> RawObserver<S> for TypedObserver<T, O> {
    fn observe(&mut self, server: &mut S) -> Vec<HandlerEntry<S>> {
        let mut new_handlers = Vec::new();
        while let Some(value) = self.1.poll() {
            let iteration_ctx = OCtx {
                new_handlers: &mut new_handlers,
            };
            server.observe(iteration_ctx, value, self.0);
        }
        new_handlers
    }
}

static CAPABILITIES_ALLOCATED: AtomicUsize = AtomicUsize::new(1);

impl<S: ?Sized> Ctx<'_, S> {
    pub fn grant_capability<T: Interface + 'static, O: Copy + 'static>(
        &mut self,
        object: O,
    ) -> Capability<T>
    where
        S: Handler<T, O>,
    {
        let certificate = allocate_certificate();
        certificate.store(
            CapabilityCertificateValue::granted(self.sender),
            Ordering::Relaxed,
        );
        let cap = Capability(RawCapability::from_pointer(certificate), PhantomData);
        let t_name = T::NAME;
        trace!("granted {cap:?} {t_name} to {:?}", self.sender);

        self.new_handlers.push(HandlerEntry {
            local_index: cap.local_index(),
            handler: Box::new(TypedHandler(object, PhantomData)),
        });

        cap
    }

    pub fn forward_capability<T: Interface>(&mut self, cap: Capability<T>) -> Capability<T> {
        forward_capability_by_pid(cap, self.sender)
    }
}

impl<S: ?Sized> OCtx<'_, S> {
    pub fn grant_capability_to_kernel<T: Interface + 'static, O: Copy + 'static>(
        &mut self,
        object: O,
    ) -> Capability<T>
    where
        S: Handler<T, O>,
    {
        let certificate = allocate_certificate();
        certificate.store(
            CapabilityCertificateValue::granted(Actor::Kernel),
            Ordering::Relaxed,
        );
        let cap = Capability(RawCapability::from_pointer(certificate), PhantomData);
        let t_name = T::NAME;
        trace!("self-granted {cap:?} {t_name}");

        self.new_handlers.push(HandlerEntry {
            local_index: cap.local_index(),
            handler: Box::new(TypedHandler(object, PhantomData)),
        });

        cap
    }
}

impl<S> Dispatch<S> {
    pub fn new<T: 'static>(server: S) -> Dispatch<S>
    where
        S: Handler<T, ()>,
    {
        Dispatch::new_object(server, ())
    }

    pub fn new_object<T: 'static, O: Copy + 'static>(server: S, object: O) -> Dispatch<S>
    where
        S: Handler<T, O>,
    {
        let handlers = vec![Some(
            Box::new(TypedHandler(object, PhantomData)) as Box<dyn RawHandler<S> + 'static>
        )];
        Dispatch {
            server,
            handlers,
            observers: Vec::new(),
        }
    }

    pub fn observe<T: Copy + 'static, O: Copy + 'static>(
        &mut self,
        object: O,
        ring: &'static RingBuffer<T>,
    ) where
        S: Observer<T, O>,
    {
        self.observers.push(Box::new(TypedObserver(object, ring)));
    }
}

impl<S> Dispatch<S> {
    pub fn dispatch(
        &mut self,
        cap: RawCapability,
        method: usize,
        args: &[u8],
        sender: ProcessId,
    ) -> Vec<u8> {
        let Some(handler) = self.handlers[cap.local_index()].as_mut() else {
            panic!(
                "dispatch on unhandled {cap:?}, method {method} {} from {sender:?}",
                str::from_utf8(args).unwrap()
            )
        };
        let (result, new_handlers) = handler.call_method(&mut self.server, method, args, sender);
        self.handlers
            .resize_with(CAPABILITIES_ALLOCATED.load(Ordering::Relaxed), || None);
        for new_handler in new_handlers {
            self.handlers[new_handler.local_index] = Some(new_handler.handler);
        }
        result
    }

    pub fn run_observables(&mut self) {
        let mut new_handlers = Vec::new();
        for observable in &mut self.observers {
            let iteration_new_handlers = observable.observe(&mut self.server);
            new_handlers.extend(iteration_new_handlers);
        }
        self.handlers
            .resize_with(CAPABILITIES_ALLOCATED.load(Ordering::Relaxed), || None);
        for new_handler in new_handlers {
            self.handlers[new_handler.local_index] = Some(new_handler.handler);
        }
    }
}

pub fn forward_capability_by_cap<T: Interface, U: Interface>(
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
    forwardee: impl Into<Actor>,
) -> Capability<T> {
    let forwardee = forwardee.into();
    let certificate = allocate_certificate();
    certificate.store(
        CapabilityCertificateValue::forwarded(forwardee, cap.0),
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
    let index = CAPABILITIES_ALLOCATED.fetch_add(1, Ordering::Relaxed);
    assert!(
        index < 4096 / size_of::<CapabilityCertificateValue>(),
        "out of capability certificate slots"
    );
    &get_capability_certificate_page(current_pid().into())[index]
}
