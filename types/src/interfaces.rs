use crate::capability::{CallableCapability, Capability};

pub trait ProcessTag {
    type Capabilities;

    type Export;

    const NAME: &'static str;
}

include!(concat!(env!("OUT_DIR"), "/interfaces.rs"));
