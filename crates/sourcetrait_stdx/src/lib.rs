pub mod convert {
    pub mod try_with;
    
    pub use try_with::{TryFromWith, TryIntoWith, TryFromTransformer};
}

/// Error and Result utilities
pub mod error {
    pub mod fs;
}
 
/// Filesystem utilities
pub mod fs {
    pub mod find;
    pub mod touch;
    
    pub use self::{find::*, touch::*};
}

pub mod option {
    pub mod either;
    
    pub use self::either::*;
}

/// Path utilities
pub mod path {
    pub mod normalize;
    pub mod tree;
}

/// Process extensions
pub mod process {
    pub mod command;
    pub use command::*;
}

pub mod str {
    pub mod as_str;
    
    pub use as_str::*;
}

pub mod ver {
    pub mod penver;
    
    pub use penver::*;
}

pub(crate) use std::{
    fmt,
    str::FromStr,
};
