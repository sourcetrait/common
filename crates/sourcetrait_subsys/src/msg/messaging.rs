use crate::*;

pub type MsgID = u64;

#[cereal::derived(Copy, Eq)]
pub enum FromSub {
    ControlResponse(Packet<ControlResponse>),
    StatusChange(Packet<StatusChange>),
    StatusResponse(Packet<StatusResponse>),
}

#[cereal::derived]
pub enum MsgToSys<T> {
    Envelope, //todo
    Packet(Packet<T>),
    Sub(ToSub),
}

#[cereal::derived]
pub enum MsgFromSys<T> {
    Envelope, // todo
    Packet(Packet<T>),
    Sub(FromSub),
}

impl<T> MsgFromSys<T> {
    pub fn take_packet(self) -> Option<Packet<T>> {
        match self {
            MsgFromSys::Packet(packet) => Some(packet),
            _ => None,
        }
    }

    pub const fn packet_response_to_id(&self) -> Option<u64> {
        if let Self::Packet(pkt) = self && let PacketNature::Response(reqid) = pkt.nature {
            Some(reqid)
        } else {
            None
        }
    }
}

#[cereal::derived(Copy, Eq)]
pub enum PacketNature {
    Singular,
    Request,
    Response(MsgID),
}

impl PacketNature {
    pub const fn response(to: MsgID) -> Self {
        Self::Response(to)
    }
}

#[cereal::derived]
pub struct Packet<T> {
    pub id: MsgID,
    pub nature: PacketNature,
    pub msg: T,
}

impl<T: Eq> Eq for Packet<T> {}
impl<T: Copy> Copy for Packet<T> {}

impl<T> Packet<T> {
    pub const fn new(id: MsgID, nature: PacketNature, msg: T) -> Self {
        Self {
            id,
            nature,
            msg,
        }
    }
    
    pub fn singular(msg: T) -> Self {
        Self {
            id: Self::next_id(),
            nature: PacketNature::Singular,
            msg,
        }
    }
    
    pub fn request(msg: T) -> Self {
        Self {
            id: Self::next_id(),
            nature: PacketNature::Request,
            msg,
        }
    }
    
    pub fn response(to: MsgID, msg: T) -> Self {
        Packet {
            id: Self::next_id(),
            nature: PacketNature::response(to),
            msg,
        }
    }
    
    pub fn respond<U>(&self, msg: U) -> Packet<U> {
        Packet {
            id: Self::next_id(),
            nature: PacketNature::response(self.id),
            msg,
        }
    }
    
    pub const fn id(&self) -> MsgID { self.id }
    
    pub const fn msg(&self) -> &T { &self.msg }

    pub fn take_msg(self) -> T { self.msg }
    
    pub const fn is_singular(&self) -> bool {
        matches!(self.nature, PacketNature::Singular)
    }
    
    pub const fn is_request(&self) -> bool {
        matches!(self.nature, PacketNature::Request)
    }
    
    pub const fn is_response(&self) -> bool {
        matches!(self.nature, PacketNature::Response(_))
    }
    
    pub fn into_tuple(self) -> (MsgID, PacketNature, T) {
        let Self { id, nature, msg } = self;
        ( id, nature, msg )
    }
    
    pub const fn response_to_id(&self) -> Option<u64> {
        if let PacketNature::Response(reqid) = self.nature {
            Some(reqid)
        } else {
            None
        }
    }
    
    fn next_id() -> MsgID {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }
}

#[cereal::derived(Eq, Data)]
pub enum ToSub {
    ControlRequest(Packet<ControlRequest>),
    StatusRequest(Packet<StatusRequest>),
}
