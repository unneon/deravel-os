use crate::{ProcessId, current_pid};
use core::sync::atomic::{AtomicUsize, Ordering};
use log::trace;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy)]
pub struct Capability(pub(crate) *const CapabilityCertificate);

#[derive(Clone, Copy)]
pub(crate) struct CapabilityCertificate(usize);

#[derive(Debug)]
enum CapabilityCertificateUnpacked {
    Granted {
        grantee: ProcessId,
    },
    Forwarded {
        forwardee: ProcessId,
        inner: Capability,
    },
}

const CAPABILITIES_START: usize = 0x2000000;
const CAPABILITIES_END: usize = 0x3000000;

static CAPABILITIES_ALLOCATED: AtomicUsize = AtomicUsize::new(0);

impl Capability {
    pub fn grant(grantee: ProcessId) -> Capability {
        let certificate = allocate_certificate();
        *certificate = CapabilityCertificate::grant(grantee);
        let cap = Capability(certificate);
        trace!("granted {cap:?} to {grantee:?}");
        cap
    }

    pub fn forward(self, forwardee: ProcessId) -> Capability {
        let certificate = allocate_certificate();
        *certificate = CapabilityCertificate::forward(forwardee, self);
        let cap = Capability(certificate);
        trace!("forwarded {self:?} as {cap:?} to {forwardee:?}");
        cap
    }

    pub fn validate(self, claimer: ProcessId) -> Capability {
        trace!("validating capability {self:?} from process {claimer:?}");
        let mut capability = self;
        let mut sender = claimer;
        let original = loop {
            assert!(capability.is_pointer_valid());
            let certifier = capability.certifier();
            match capability.certificate().unpack() {
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
        assert!(original.certifier() == current_pid());
        original
    }

    pub fn local_index(self) -> usize {
        assert!(self.is_pointer_valid());
        (self.0 as usize % 4096) / 8
    }

    fn certificate(self) -> CapabilityCertificate {
        assert!(self.is_pointer_valid());
        unsafe { *self.0 }
    }

    fn certifier(self) -> ProcessId {
        assert!(self.is_pointer_valid());
        ProcessId((self.0 as usize - CAPABILITIES_START) / 4096)
    }

    fn is_pointer_valid(self) -> bool {
        let in_range = (CAPABILITIES_START..CAPABILITIES_END).contains(&(self.0 as usize));
        let aligned = self.0.is_aligned();
        in_range && aligned
    }
}

impl CapabilityCertificate {
    fn grant(grantee: ProcessId) -> CapabilityCertificate {
        assert!(grantee.0 < 8);
        CapabilityCertificate(grantee.0)
    }

    fn forward(forwardee: ProcessId, capability: Capability) -> CapabilityCertificate {
        assert!(forwardee.0 < 8);
        assert!(capability.0.is_aligned_to(8));
        CapabilityCertificate(forwardee.0 | capability.0 as usize)
    }

    fn unpack(self) -> CapabilityCertificateUnpacked {
        let certified = ProcessId(self.0 % 8);
        let raw_inner = self.0 ^ certified.0;
        if raw_inner == 0 {
            CapabilityCertificateUnpacked::Granted { grantee: certified }
        } else {
            CapabilityCertificateUnpacked::Forwarded {
                forwardee: certified,
                inner: Capability(raw_inner as *const CapabilityCertificate),
            }
        }
    }
}

impl core::fmt::Debug for Capability {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0 as usize)
    }
}

impl<'de> Deserialize<'de> for Capability {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Capability(
            usize::deserialize(deserializer)? as *const CapabilityCertificate
        ))
    }
}

impl Serialize for Capability {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.0 as usize).serialize(serializer)
    }
}

fn allocate_certificate() -> &'static mut CapabilityCertificate {
    let index = CAPABILITIES_ALLOCATED.fetch_add(1, Ordering::Relaxed);
    assert!(
        index < 4096 / size_of::<CapabilityCertificate>(),
        "out of capability certificate slots"
    );
    let all_certificates = CAPABILITIES_START as *mut CapabilityCertificate;
    let our_certificates = unsafe { all_certificates.byte_add(4096 * current_pid().0) };
    let certificate = unsafe { our_certificates.add(index) };
    unsafe { &mut *certificate }
}
