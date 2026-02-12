#![no_std]
extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use deravel_kernel_api::Capability;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub enum FilesystemRequest {
    Read {
        cap: Capability,
        path: String,
    },
    Write {
        cap: Capability,
        path: String,
        data: Vec<u8>,
    },
    Subcapability {
        cap: Capability,
        path: String,
    },
}
