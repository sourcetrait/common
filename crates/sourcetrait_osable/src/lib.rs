pub(crate) mod error;
pub(crate) mod os {
    #[cfg(target_family = "unix")]
    pub(crate) mod unix;
    #[cfg(target_os = "linux")]
    pub(crate) mod linux;
    #[cfg(target_os = "macos")]
    pub(crate) mod macos;
    #[cfg(target_os = "windows")]
    pub(crate) mod windows;
}
pub(crate) mod extend;

#[cfg(target_family = "unix")]
pub mod unix {
    pub use crate::os::unix::{
        UID, GID,
        mkdtemp,
        uid, gid, username_id, groupname_id, username, groupname,
        effective_username, effective_groupname,
    };
}

pub(crate) use crate::{
    error::{OsableError, OsableResult},
    extend::{OsableFromUtf8, OsableIntoUtf8},
};

pub(crate) use std::{
    path::{Path, PathBuf},
    ffi::{CString, CStr},
    io, mem,
};
