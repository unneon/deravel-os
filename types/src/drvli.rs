#![allow(non_camel_case_types, unused)]

use crate::SharedMemory;
use crate::capability::{Capability, RawCapability};
use core::fmt::Debug;
use serde::{Deserialize, Serialize};

pub trait Interface {
    const NAME: &'static str;
}

pub trait ProcessArgs: Debug + for<'a> Deserialize<'a> {
    type Process: ProcessTag<Args = Self>;

    fn for_all(&self, f: impl FnMut(RawCapability));
}

pub trait ProcessTag {
    type Args: ProcessArgs;

    type Export;

    type Spawner;

    const NAME: &'static str;
}

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
