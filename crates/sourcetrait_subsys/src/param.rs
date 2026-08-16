use crate::*;

/// Static configuration typically loaded from an IO resource (file).
pub trait Params: Sized + Debug + Clone + serde::Serialize + serde::de::DeserializeOwned {}
