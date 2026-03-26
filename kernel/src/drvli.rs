#![allow(clippy::let_unit_value)]
#![allow(clippy::match_single_binding)]

use alloc::string::String;
use alloc::vec::Vec;
use deravel_types::*;

pub trait Handler<T> {
    fn handle(&self, method: usize, args: &[u8]) -> Vec<u8>;
}

pub trait RawHandler {
    fn handle(&self, method: usize, args: &[u8]) -> Vec<u8>;
}

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
