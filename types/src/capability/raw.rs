use crate::capability::pages::{
    CAPABILITIES_END, CAPABILITIES_START, get_capability_certificate_page,
};
use crate::{Actor, CapabilityCertificate, PAGE_SIZE, ProcessId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct RawCapability(&'static CapabilityCertificate);

impl RawCapability {
    pub fn new(certifier: impl Into<Actor>, local_index: usize) -> RawCapability {
        assert!(local_index < PAGE_SIZE / size_of::<CapabilityCertificate>());
        RawCapability(&get_capability_certificate_page(certifier.into())[local_index])
    }

    pub fn from_ref(ptr: &'static CapabilityCertificate) -> RawCapability {
        assert!(is_ptr_valid(ptr));
        RawCapability(ptr)
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn from_ptr(ptr: *const CapabilityCertificate) -> RawCapability {
        assert!(is_ptr_valid(ptr));
        RawCapability(unsafe { &*ptr })
    }

    pub fn certifier(self) -> Actor {
        let page_index = (self.as_usize() - CAPABILITIES_START) / PAGE_SIZE;
        if page_index == 0 {
            Actor::Kernel
        } else {
            Actor::Userspace(ProcessId::new(page_index as u16))
        }
    }

    pub fn local_index(self) -> usize {
        (self.as_usize() % PAGE_SIZE) / 8
    }

    pub fn as_usize(self) -> usize {
        self.0 as *const CapabilityCertificate as usize
    }
}

impl core::fmt::Debug for RawCapability {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.as_usize())
    }
}

impl<'de> Deserialize<'de> for RawCapability {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let cap = usize::deserialize(deserializer)? as *const CapabilityCertificate;
        Ok(RawCapability::from_ptr(cap))
    }
}

impl Serialize for RawCapability {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_usize().serialize(serializer)
    }
}

fn is_ptr_valid(maybe_cap: *const CapabilityCertificate) -> bool {
    let in_range = (CAPABILITIES_START..CAPABILITIES_END).contains(&(maybe_cap as usize));
    let aligned = maybe_cap.is_aligned();
    in_range && aligned
}
