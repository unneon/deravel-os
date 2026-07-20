#![allow(clippy::diverging_sub_expression)]
#![allow(clippy::let_unit_value)]
#![allow(clippy::match_single_binding)]

use crate::RiscvRegisters;
use crate::capability::Handler;
use crate::stack::UserCtx;
use crate::user::{UserPtr, UserSyscallError};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use deravel_types::abi::to_reg;
use deravel_types::*;

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
