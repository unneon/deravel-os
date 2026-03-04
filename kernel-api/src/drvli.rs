#![allow(
    unused,
    clippy::let_unit_value,
    clippy::match_single_binding,
    clippy::never_loop
)]

use crate::syscall::{ipc_call, ipc_receive, ipc_reply};
use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;
use deravel_types::*;

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
