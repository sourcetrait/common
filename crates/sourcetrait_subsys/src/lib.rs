pub(crate) mod channel;
pub(crate) mod config;
pub(crate) mod control;
pub(crate) mod error;
pub(crate) mod inner;
pub(crate) mod param;
pub(crate) mod msg {
    pub(crate) mod control;
    pub(crate) mod messaging;
    pub(crate) mod status;
}
pub(crate) mod status;
pub(crate) mod sys_io;
pub(crate) mod system;

pub use crate::{
    channel::*,
    config::*,
    control::*,
    error::*,
    inner::*,
    param::*,
    msg::{
        control::*,
        messaging::*,
        status::*,
    },
    status::*,
    sys_io::*,
    system::*,
};

pub mod prelude {
    pub use crate::control::{Handler, Request};
    pub use crate::{
        error::ExitTrait,
        system::System,
    };
}

#[allow(unused_imports)]
pub(crate) use crate::{
    channel::*,
    config::*,
    error::*,
    sys_io::*,
    system::*,
};

#[allow(unused_imports)]
pub(crate) use std::{
    fmt::Debug,
    io::{self, stdin},
    hash::Hash,
    path::{PathBuf, Path},
    process::ExitCode,
    sync::{
        atomic::AtomicU64,
        Arc, Mutex, MutexGuard
    },
};

pub(crate) mod cereal {
    pub(crate) use sourcetrait_cereal::*;
    pub(crate) use sourcetrait_cereal_macro::*;
}
pub(crate) use sourcetrait_agnostic as agnostic;
pub(crate) mod tkio {
    pub(crate) use tokio::sync::mpsc;
    pub(crate) use tokio::task;
    pub(crate) use tokio_util::sync::CancellationToken;
    pub(crate) mod oneshot {
        #[allow(unused)]
        pub(crate) use tokio::sync::oneshot::{Receiver, Sender, channel};
    }
}

pub(crate) use rustc_hash::FxHashMap;