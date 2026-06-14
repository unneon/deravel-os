use crate::{Actor, CapabilityCertificate, MAX_PROCESSES, PAGE_SIZE};

#[repr(align(4096))]
pub struct CapabilityPage(pub [CapabilityCertificate; CAPABILITIES_PER_PAGE]);

pub const CAPABILITIES_PER_PAGE: usize = PAGE_SIZE / size_of::<CapabilityCertificate>();

pub const CAPABILITIES_START: usize = 0x2000000;
pub const CAPABILITIES_END: usize = 0x3000000;

pub fn get_capability_certificate_page(
    actor: Actor,
) -> &'static [CapabilityCertificate; CAPABILITIES_PER_PAGE] {
    let offset = match actor {
        Actor::Userspace(pid) => pid.as_u16() as usize,
        Actor::Kernel => 0,
    };
    &pages()[offset]
}

fn pages() -> &'static [[CapabilityCertificate; CAPABILITIES_PER_PAGE]; MAX_PROCESSES + 1] {
    unsafe { &*(CAPABILITIES_START as *const _) }
}
