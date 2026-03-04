use crate::capability::Capability;

pub trait ProcessTag {
    type Capabilities;

    type Export;

    const NAME: &'static str;
}

include!(concat!(env!("OUT_DIR"), "/drvli.rs"));
