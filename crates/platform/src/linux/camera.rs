//! Linux has no camera privacy switch: a device node's permissions show up as an open error that
//! names the node (research R6).

use crate::camera::Access;

pub fn access() -> Access {
    Access::Unknown
}

pub fn open_privacy() {}
