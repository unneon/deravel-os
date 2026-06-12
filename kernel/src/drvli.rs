#![allow(clippy::let_unit_value)]
#![allow(clippy::match_single_binding)]

use crate::RiscvRegisters;
use crate::capability::Handler;
use crate::hart::HartContext;
use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;
use deravel_types::syscall::to_reg;
use deravel_types::*;

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
