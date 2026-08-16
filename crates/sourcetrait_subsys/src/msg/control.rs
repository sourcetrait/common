use crate::*;

#[cereal::derived(Copy, Eq)]
pub struct ControlRequest {
    pub control: Control,
}

pub type ControlResult = RunResult; 

#[cereal::derived(Copy, Eq)]
pub struct ControlResponse {
    pub result: ControlResult,
}

#[cereal::derived(Copy, Eq)]
pub enum Control {
    Drain,
    Drop,
    Refresh,
    Restart,
    /// Resume accepting new work and/or new connections.
    Resume,
    /// Shutdown
    Stop { halt: bool },
}
