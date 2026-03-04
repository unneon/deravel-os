use crate::CallableCapability;
use crate::alloc::vec::Vec;
use crate::syscall::ipc_call;
use deravel_types::capability::Capability;
use deravel_types::drvli::*;

pub trait App {
    type Args;
}

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
