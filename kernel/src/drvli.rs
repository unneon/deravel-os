#![allow(clippy::diverging_sub_expression)]
#![allow(clippy::let_unit_value)]
#![allow(clippy::match_single_binding)]

use crate::capability::Handler;
use crate::heap::MutAllocator;
use crate::page::{PageFlags, PageTable, virt_to_phys};
use crate::process::Process;
use crate::stack::UserCtx;
use crate::user::{UserPtr, UserSyscallError};
use crate::virtual_memory::VirtualMemoryRawMapping;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::ops::Range;
use deravel_types::abi::to_reg;
use deravel_types::*;

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
