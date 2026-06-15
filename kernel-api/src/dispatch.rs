use crate::capability::CAPABILITIES_ALLOCATED;
use crate::{forward, grant_unhandled, syscall, yield_};
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::sync::atomic::Ordering;
use deravel_types::{Actor, Capability, Interface, ProcessId, RawCapability, RingBuffer};

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

impl<S: ?Sized> Ctx<'_, S> {
    pub fn grant_to_sender<T: Interface + 'static, O: Copy + 'static>(
        &mut self,
        object: O,
    ) -> Capability<T>
    where
        S: Handler<T, O>,
    {
        let cap = grant_unhandled(self.sender);
        self.new_handlers.push(HandlerEntry {
            local_index: cap.local_index(),
            handler: Box::new(TypedHandler(object, PhantomData)),
        });
        cap
    }

    pub fn forward_to_sender<T: Interface>(&mut self, cap: Capability<T>) -> Capability<T> {
        forward(cap, self.sender)
    }
}

impl<S: ?Sized> OCtx<'_, S> {
    pub fn grant_to_kernel<T: Interface + 'static, O: Copy + 'static>(
        &mut self,
        object: O,
    ) -> Capability<T>
    where
        S: Handler<T, O>,
    {
        let cap = grant_unhandled(Actor::Kernel);
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
        let handlers = vec![Some(
            Box::new(TypedHandler((), PhantomData)) as Box<dyn RawHandler<S> + 'static>
        )];
        Dispatch {
            server,
            handlers,
            observers: Vec::new(),
        }
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

    pub fn run(&mut self) -> ! {
        loop {
            self.run_calls();
            self.run_observables();
            yield_();
        }
    }

    fn run_calls(&mut self) {
        loop {
            let mut buf = [0u8; 4096];
            let (cap, method, args_len, sender) =
                unsafe { syscall::ipc_receive(buf.as_mut_ptr(), buf.len()) };
            let (Some(cap), Some(sender)) = (cap, sender) else {
                break;
            };
            let result = self.run_call(cap, method, &buf[..args_len], sender);
            unsafe { syscall::ipc_reply(result.as_ptr(), result.len()) }
        }
    }

    fn run_call(
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

    fn run_observables(&mut self) {
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
