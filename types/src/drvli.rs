#![allow(non_camel_case_types, unused)]

use crate::capability::{Capability, RawCapability};

pub trait ProcessArgs {
    fn for_all(&self, f: impl FnMut(RawCapability));
}

pub trait ProcessTag {
    type Args: ProcessArgs;

    type Export;

    const NAME: &'static str;
}

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
