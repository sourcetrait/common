use crate::*;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    serde::Serialize, serde::Deserialize,
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
    bitcode::Encode, bitcode::Decode,
)]
pub enum Status {
    /// New work and connections are being accepted
    Ready,
    /// No new work or connections are being accepted
    NotReady(NotReady),
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    serde::Serialize, serde::Deserialize,
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
    bitcode::Encode, bitcode::Decode,
)]
pub enum NotReady {
    /// The system is starting up or restarting
    Normal,
    /// The system is completing its work queue
    Drain,
    /// The system is dropping its work queue
    Drop,
    /// The system is refreshing resources
    Refresh,
    /// The system has is waiting for the controller to tell it to resume
    Pause,
    /// The system is stopping
    Stop { halt: bool },
}
