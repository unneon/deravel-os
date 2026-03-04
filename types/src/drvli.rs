#![allow(non_camel_case_types, unused)]

use crate::capability::Capability;

pub trait CapabilityContainer {
    fn for_all(&self, f: impl FnMut(Capability));
}

pub trait ProcessTag {
    type Capabilities: CapabilityContainer;

    type Export;

    const NAME: &'static str;
}

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
