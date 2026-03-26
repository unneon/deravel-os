use crate::sync::Mutex;
use core::sync::atomic::{AtomicUsize, Ordering};

pub trait InterruptHandler {
    fn handle(&self);
}

#[derive(Clone, Copy)]
pub struct InterruptEntry {
    pub plic_number: u32,
    pub handler: &'static (dyn InterruptHandler + Send + Sync),
}

const MAX_INTERRUPT_HANDLERS: usize = 16;

static ALLOCATED_COUNT: AtomicUsize = AtomicUsize::new(0);

pub static INTERRUPTS: [Mutex<Option<InterruptEntry>>; MAX_INTERRUPT_HANDLERS] =
    [const { Mutex::new(None) }; _];

pub fn register_interrupt(
    plic_number: u32,
    handler: &'static (dyn InterruptHandler + Send + Sync),
) {
    let index = ALLOCATED_COUNT.fetch_add(1, Ordering::Relaxed);
    assert!(index < MAX_INTERRUPT_HANDLERS);
    *INTERRUPTS[index].lock() = Some(InterruptEntry {
        plic_number,
        handler,
    });
}
