pub trait InterruptHandler {
    fn handle(&self);
}

#[derive(Clone, Copy)]
pub struct InterruptEntry {
    pub plic_number: u32,
    pub handler: &'static dyn InterruptHandler,
}

const MAX_INTERRUPT_HANDLERS: usize = 16;

pub static mut INTERRUPTS: [Option<InterruptEntry>; MAX_INTERRUPT_HANDLERS] = [None; _];
static mut INTERRUPT_COUNTER: usize = 0;

pub fn register_interrupt(plic_number: u32, handler: &'static dyn InterruptHandler) {
    let index = unsafe { INTERRUPT_COUNTER };
    assert!(index < MAX_INTERRUPT_HANDLERS);
    unsafe { INTERRUPT_COUNTER += 1 }
    unsafe {
        INTERRUPTS[index] = Some(InterruptEntry {
            plic_number,
            handler,
        })
    }
}
