#![allow(non_camel_case_types, unused)]

use crate::capability::{Capability, RawCapability};
use serde::{Deserialize, Serialize};

pub trait Interface {
    const NAME: &'static str;
}

pub trait ProcessArgs {
    fn for_all(&self, f: impl FnMut(RawCapability));
}

pub trait ProcessTag {
    type Args: ProcessArgs;

    type Export;

    const NAME: &'static str;
}

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
