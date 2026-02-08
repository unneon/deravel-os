use crate::{ProcessId, current_pid, println};
use core::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy)]
pub struct Capability(usize);

#[derive(Debug)]
pub enum CapabilityCertificate {
    Granted {
        grantee: ProcessId,
    },
    Forwarded {
        forwardee: ProcessId,
        inner: Capability,
    },
}

#[derive(Clone, Copy)]
pub struct CapabilityCertificatePacked(usize);

const CAPABILITIES_START: usize = 0x2000000;
const CAPABILITIES_END: usize = 0x3000000;

pub static CAPABILITIES_ALLOCATED: AtomicUsize = AtomicUsize::new(0);

impl Capability {
    pub fn grant(grantee: ProcessId) -> Capability {
        let certificate = allocate_certificate();
        *certificate = CapabilityCertificatePacked::grant(grantee);
        Capability(certificate as *mut CapabilityCertificatePacked as usize)
    }

    pub fn forward(self, forwardee: ProcessId) -> Capability {
        let certificate = allocate_certificate();
        *certificate = CapabilityCertificatePacked::forward(forwardee, self);
        Capability(certificate as *mut CapabilityCertificatePacked as usize)
    }

    pub fn validate(self, claimer: ProcessId) -> Capability {
        println!("validating capability {self:?} from process {claimer:?}");
        assert!(self.is_in_range());
        let mut capability = self;
        let mut sender = claimer;
        let original = loop {
            let certifier = capability.certifier();
            match capability.read_export().unpack() {
                CapabilityCertificate::Granted { grantee } => {
                    println!("... granted from {certifier:?} to {grantee:?}");
                    assert_eq!(grantee, sender);
                    break capability;
                }
                CapabilityCertificate::Forwarded { forwardee, inner } => {
                    println!("... was {inner:?} forwarded from {certifier:?} to {forwardee:?}");
                    assert_eq!(forwardee, sender);
                    sender = certifier;
                    capability = inner;
                }
            }
        };
        assert_eq!(original.certifier(), current_pid());
        original
    }

    fn read_export(self) -> CapabilityCertificatePacked {
        assert!(self.is_in_range());
        unsafe { *(self.0 as *const CapabilityCertificatePacked) }
    }

    fn certifier(self) -> ProcessId {
        assert!(self.is_in_range());
        ProcessId((self.0 - CAPABILITIES_START) / 4096)
    }

    fn is_in_range(self) -> bool {
        (CAPABILITIES_START..CAPABILITIES_END).contains(&self.0)
    }
}

impl CapabilityCertificatePacked {
    fn grant(grantee: ProcessId) -> CapabilityCertificatePacked {
        assert!(grantee.0 < 8);
        CapabilityCertificatePacked(grantee.0)
    }

    fn forward(forwardee: ProcessId, capability: Capability) -> CapabilityCertificatePacked {
        assert!(forwardee.0 < 8);
        assert!(capability.0.is_multiple_of(8));
        CapabilityCertificatePacked(forwardee.0 | capability.0)
    }

    fn unpack(self) -> CapabilityCertificate {
        let certified = ProcessId(self.0 % 8);
        let raw_inner = self.0 ^ certified.0;
        if raw_inner == 0 {
            CapabilityCertificate::Granted { grantee: certified }
        } else {
            CapabilityCertificate::Forwarded {
                forwardee: certified,
                inner: Capability(raw_inner),
            }
        }
    }
}

impl core::fmt::Debug for Capability {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

fn allocate_certificate() -> &'static mut CapabilityCertificatePacked {
    let index = CAPABILITIES_ALLOCATED.fetch_add(1, Ordering::Relaxed);
    assert!(
        index < 4096 / size_of::<CapabilityCertificatePacked>(),
        "out of capability certificate slots"
    );
    let all_certificates = CAPABILITIES_START as *mut CapabilityCertificatePacked;
    let our_certificates = unsafe { all_certificates.byte_add(4096 * current_pid().0) };
    let certificate = unsafe { our_certificates.add(index) };
    unsafe { &mut *certificate }
}
