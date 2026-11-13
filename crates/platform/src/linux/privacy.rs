//! Linux has no camera or microphone privacy switch: a device's permissions show up as an open
//! error that names it (device preview research R6, preview audio R7).

use crate::privacy::{Access, Capability};

pub fn access(_: Capability) -> Access {
    Access::Unknown
}

pub fn open_privacy(_: Capability) {}
