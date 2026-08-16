use crate::*;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    serde::Serialize, serde::Deserialize,
    bitcode::Encode, bitcode::Decode,
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
pub struct StatusRequest;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    serde::Serialize, serde::Deserialize,
    bitcode::Encode, bitcode::Decode,
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
pub struct StatusResponse {
    pub status: Status,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    serde::Serialize, serde::Deserialize,
    bitcode::Encode, bitcode::Decode,
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
pub struct StatusChange(pub Status);
