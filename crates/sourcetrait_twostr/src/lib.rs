pub(crate) mod error;
pub(crate) mod twoc;
pub(crate) mod twostr;
pub(crate) mod twostring;
pub(crate) mod astwo;

pub use crate::{
    astwo::*,
    error::*,
    twoc::*,
    twostr::*,
    twostring::*,
};

pub mod prelude {
    pub use crate::{AsTwoStr, TwoStrC, TwoStringC};
}

pub(crate) use std::{
    borrow::{Borrow, Cow},
    ffi::{CStr, CString, OsStr, OsString},
};

#[cfg(target_family = "unix")]
pub(crate) mod unix {
    pub(crate) mod twoc_str;
    pub(crate) mod twoc_string;
    
    pub(crate) use std::os::unix::ffi::OsStrExt;
}

#[cfg(target_family = "windows")]
pub(crate) mod windows {
    pub(crate) mod twoc_str;
    pub(crate) mod twoc_string;
}
