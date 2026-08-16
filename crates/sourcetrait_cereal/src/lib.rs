pub(crate) mod data;
pub(crate) mod base62;
pub mod bitcoded;

pub use crate::{
    data::{
        Data,
        DataCopy,
        DataEq,
        DataCopyEq,
        Archive,
    },
    base62::{
        Base62u64,
        is_base62,
    },
};

pub mod prelude {
    pub use crate::{
        Data,
        DataCopy,
        DataEq,
        DataCopyEq,
        Archive,
    };
}

pub(crate) use std::{
    fmt::Debug,
};

pub use xxhash_rust::{
    const_xxh3::xxh3_64 as hash64,
    const_xxh3::xxh3_128 as hash128,
    xxh3::Xxh3Default as DefaultHasher,
    xxh3::Xxh3 as Hasher,
};
