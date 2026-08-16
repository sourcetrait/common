use crate::*;

pub trait Data:
    'static + Sized + Send + Debug + Clone + PartialEq
    + serde::Serialize + serde::de::DeserializeOwned
    + bitcode::Encode + bitcode::DecodeOwned
    + Archive {}

pub trait DataEq: Data + Eq {}

pub trait DataCopy: Data + Copy {}

pub trait DataCopyEq: DataCopy + DataEq {}

pub trait Archive:
    Sized
    + rkyv::Archive<Archived: rkyv::Deserialize<Self, rkyv::api::high::HighDeserializer<rkyv::rancor::Error>>>
    + for<'a> rkyv::Serialize<rkyv::api::high::HighSerializer<rkyv::util::AlignedVec, rkyv::ser::allocator::ArenaHandle<'a>, rkyv::rancor::Error>>
{}

impl<T> Archive for T
where
    T: rkyv::Archive
        + for<'a> rkyv::Serialize<rkyv::api::high::HighSerializer<rkyv::util::AlignedVec, rkyv::ser::allocator::ArenaHandle<'a>, rkyv::rancor::Error>>,
    T::Archived: rkyv::Deserialize<T, rkyv::api::high::HighDeserializer<rkyv::rancor::Error>>,
{}

impl<T> Data for T
where
    T: 'static + Sized + Send + Debug + Clone + PartialEq
        + serde::Serialize + serde::de::DeserializeOwned
        + bitcode::Encode + bitcode::DecodeOwned
        + Archive
{}
    
impl<T> DataCopy for T where T: Data + Copy {}

impl<T> DataEq for T where T: Data + Eq {}

impl<T> DataCopyEq for T where T: Data + Copy + Eq {}

