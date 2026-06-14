#![allow(unused)]
#![allow(clippy::diverging_sub_expression)]
#![allow(clippy::let_unit_value)]
#![allow(clippy::match_single_binding)]
#![allow(clippy::never_loop)]
#![allow(clippy::too_many_arguments)]

use crate::abi::*;
use crate::{Ctx, Handler, RingBuffer};
use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;
use deravel_types::*;

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
