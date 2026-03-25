#![allow(clippy::let_unit_value)]
#![allow(clippy::match_single_binding)]

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use deravel_types::*;

pub trait Handler {
    fn handle(&self, method: usize, args: &[u8]) -> Vec<u8>;
}

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
