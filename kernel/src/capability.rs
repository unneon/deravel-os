use crate::drvli::Handler;
use alloc::boxed::Box;
use deravel_types::{CapabilityCertificate, PAGE_SIZE};

pub static mut HANDLERS: [Option<Box<dyn Handler>>;
    PAGE_SIZE / size_of::<CapabilityCertificate>()] = [const { None }; _];
