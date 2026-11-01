pub mod access {
    pub mod cache;
    pub mod component;
}
pub mod capable;
pub mod caching;
pub mod cross {
    pub mod component {
        pub mod access;
        pub mod cmd;
        pub mod files;
        pub mod net;
        pub mod paths;
        pub mod ui;
    }
    pub mod cross;
    pub mod standard;
    pub mod platform;
}
pub mod dir {
    pub mod app_paths;
    pub mod cross;
    pub mod xdg;
}
pub mod error;
pub mod platform {
    #[cfg(target_os = "linux")]
    pub mod linux_platform;
    
    #[cfg(target_os = "macos")]
    pub mod macos_platform;
    
    #[cfg(target_os = "windows")]
    pub mod windows_platform;
}

#[allow(unused)]
pub(crate) use std::{
    borrow::{Cow,Borrow},
    collections::{HashSet, HashMap},
    env,
    ffi::{OsString, OsStr, CStr, CString},
    fmt::Debug,
    fs,
    io,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    ptr,
    process::{self, Command},
    ops::{BitAnd, BitOr},
    str::FromStr,
    sync::{Arc, Mutex, MutexGuard, OnceLock, LazyLock},
    time::Instant,
};
pub(crate) use self::{
    access::{
        cache::*,
    },
    caching::*,
};

pub use self::{
    access::{
        component::*,
    },
    capable::*,
    cross::{
        component::{
            access::*,
            cmd::*,
            files::*,
            net::*,
            paths::*,
            ui::*,
        },
        cross::*,
        platform::*,
    },
    dir::{
        cross::*,
        xdg::*,
        app_paths::*,
    },
    error::*,
};

pub mod prelude {
    pub use crate::{
        AsAccess,
        AsAID,
        HasAccessIdent,
        UserTrait,
        UserGroupTrait,
        CrossPlatform,
        AccessComponentTrait,
        CmdComponentTrait,
        NetComponentTrait,
        PathsComponentTrait,
        FilesComponentTrait,
        UiComponentTrait,
        HasAppPaths,
        AppPaths,
    };
}

pub use sourcetrait_stdx::{
    self as stdx,
    option::Either,
};
pub use sourcetrait_agnostic_bridge::*;
pub use sourcetrait_twostr::*;

#[cfg(target_family = "unix")]
pub(crate) use sourcetrait_agnostic_unix as unix;

#[cfg(target_os = "linux")]
pub(crate) use sourcetrait_agnostic_linux as linux;
#[cfg(target_os = "linux")]
pub use crate::platform::linux_platform::*;

#[cfg(target_os = "macos")]
pub(crate) use sourcetrait_agnostic_macos as macos;
#[cfg(target_os = "macos")]
pub use crate::platform::macos_platform::*;

#[cfg(target_os = "windows")]
pub(crate) use sourcetrait_agnostic_windows as windows;
#[cfg(target_os = "windows")]
pub use crate::platform::windows_platform::*;







