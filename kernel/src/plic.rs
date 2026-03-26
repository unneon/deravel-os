use crate::util::volatile::{Volatile, volatile_struct};
use core::sync::atomic::{AtomicPtr, Ordering};
use fdt::Fdt;

volatile_struct! { Plic
    // 0x000000
    priority: ReadWrite [u32; MAX_SOURCES],
    // 0x001000
    pending: Readonly [u32; MAX_SOURCES / 32],
    _pad0: Readonly [u8; 0x002000 - 0x001000 - size_of::<u32>() * MAX_SOURCES / 32],
    // 0x002000
    enable: ReadWrite [[u32; MAX_SOURCES / 32]; MAX_CONTEXTS],
    _pad1: Readonly [u8; 0x200000 - 0x002000 - size_of::<u32>() * MAX_SOURCES / 32 * MAX_CONTEXTS],
    // 0x200000
    contexts: ReadWrite [Context; MAX_CONTEXTS],
}

volatile_struct! { Context
    priority_threshold: ReadWrite u32,
    claim_complete: ReadWrite u32,
    _reserved: Readonly [u8; 0x1000 - 8],
}

const MAX_CONTEXTS: usize = 15872;
const MAX_SOURCES: usize = 1024;

static PLIC: AtomicPtr<Plic> = AtomicPtr::null();

pub fn initialize_plic(device_tree: &Fdt) {
    let plic = find_plic(device_tree).unwrap();
    for e in 1..MAX_SOURCES {
        plic.priority().index(e).write(1);
    }
    for i in 0..5 {
        plic.enable().index(1).index(i).write(!0);
    }
    plic.contexts().index(1).priority_threshold().write(0);
}

pub fn plic_claim() -> u32 {
    get_plic().contexts().index(1).claim_complete().read()
}

pub fn plic_complete(irq: u32) {
    get_plic().contexts().index(1).claim_complete().write(irq);
}

fn find_plic(device_tree: &Fdt) -> Option<Volatile<Plic>> {
    let address = device_tree
        .find_node("/soc/plic")?
        .reg()?
        .next()?
        .starting_address;
    PLIC.store(address as *mut Plic, Ordering::Relaxed);
    Some(unsafe { Volatile::new(address as *mut Plic) })
}

fn get_plic() -> Volatile<Plic> {
    let address = PLIC.load(Ordering::Relaxed);
    assert!(!address.is_null());
    unsafe { Volatile::new(address) }
}
