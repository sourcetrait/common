pub(crate) mod styles {
    pub(crate) mod srctrait;
}
pub mod subcmd {
    pub mod cli;
}

pub mod style {
    pub use crate::styles::srctrait::set::SRCTRAIT;
    pub mod srctrait {
        pub use crate::styles::srctrait::{
            item::*,
            set::SRCTRAIT as STYLE,
            util::{
                exit_error,
            },
        };
    }
}

pub mod prelude {
    pub mod srctrait {
        pub use crate::styles::srctrait::item::*;
    }
}

pub(crate) use std::{
    error::Error,
};

pub(crate) use clap::builder::styling::{
    Styles, AnsiColor, Effects, Style
};
