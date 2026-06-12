#![allow(
    unused,
    clippy::diverging_sub_expression,
    clippy::let_unit_value,
    clippy::match_single_binding,
    clippy::never_loop
)]

use crate::syscall::*;
use crate::{Ctx, Handler, RingBuffer};
use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;
use deravel_types::*;

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
