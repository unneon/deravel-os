#![allow(
    unused,
    clippy::let_unit_value,
    clippy::match_single_binding,
    clippy::never_loop
)]

use crate::syscall::{ipc_call, ipc_receive, ipc_reply};
use alloc::string::String;
use alloc::vec::Vec;
use deravel_types::ProcessId;
use deravel_types::capability::{Capability, RawCapability};
use deravel_types::drvli::*;

pub trait App {
    type Args;
}

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
