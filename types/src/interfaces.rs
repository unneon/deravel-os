use crate::capability::{CallableCapability, Capability};

pub trait ProcessTag {
    type Capabilities;
}

include!(concat!(env!("OUT_DIR"), "/interfaces.rs"));
