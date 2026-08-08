use crate::memory::USER_CAPABILITIES;
use crate::{Actor, CapabilityCertificate, MAX_PROCESSES, PAGE_SIZE};

#[repr(align(4096))]
pub struct CapabilityPage(pub [CapabilityCertificate; CAPABILITIES_PER_PAGE]);

pub const CAPABILITIES_PER_PAGE: usize = PAGE_SIZE / size_of::<CapabilityCertificate>();

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
    unsafe { &*(USER_CAPABILITIES.start as *const _) }
}
