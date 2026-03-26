#![allow(clippy::let_unit_value)]
#![allow(clippy::match_single_binding)]

use crate::capability::Handler;
use alloc::string::String;
use alloc::vec::Vec;
use deravel_types::*;

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
