use crate::heap::MutAllocator;
use crate::heap::bitmap::BitmapAllocator;
use crate::sync::Mutex;
use alloc::sync::Arc;
use core::alloc::Layout;

pub trait InterruptHandler {
    fn handle(&self);
}

#[derive(Clone)]
struct InterruptEntry {
    plic_number: u32,
    handler: Arc<dyn InterruptHandler + Send + Sync>,
}

const MAX_INTERRUPT_HANDLERS: usize = 16;

const SLOT_LAYOUT: Layout = Layout::from_size_align(1, 1).ok().unwrap();

static ALLOCATOR: Mutex<BitmapAllocator<[usize; 1]>> =
    Mutex::new(BitmapAllocator::new(0..MAX_INTERRUPT_HANDLERS, [0]));

static INTERRUPTS: [Mutex<Option<InterruptEntry>>; MAX_INTERRUPT_HANDLERS] =
    [const { Mutex::new(None) }; _];

pub fn register_interrupt(plic_number: u32, handler: Arc<dyn InterruptHandler + Send + Sync>) {
    let index = ALLOCATOR.lock().alloc(SLOT_LAYOUT).unwrap();
    *INTERRUPTS[index].lock() = Some(InterruptEntry {
        plic_number,
        handler,
    });
}

pub fn dispatch_interrupt(irq: u32) {
    for ie in &INTERRUPTS {
        let ie = ie.lock();
        if let Some(ie) = ie.as_ref()
            && ie.plic_number == irq
        {
            ie.handler.handle();
        }
    }
}
