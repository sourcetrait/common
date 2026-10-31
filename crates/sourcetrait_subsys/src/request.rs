use crate::*;

/// The waiters for a system's requests of its controller, by request id.
type Waiters<T> = Mutex<FxHashMap<MsgID, tkio::oneshot::Sender<SubsysResult<T>>>>;

/// The answers a system awaits from its controller, owned by the system.
pub(crate) type Asked<T> = Arc<Waiters<T>>;

/// What a system asks its controller, and the answer it takes back.
pub trait Ask<SYS: System>: Into<SYS::FromSys> {
    type Answer: TryFrom<SYS::ToSys, Error = SubsysError>;
}

/// A system's handle for asking its controller, holding the system weakly.
pub struct Requester<SYS: System> {
    tx: tkio::mpsc::WeakSender<MsgFromSys<SYS::FromSys>>,
    asked: std::sync::Weak<Waiters<SYS::ToSys>>,
}

impl<SYS: System> Clone for Requester<SYS> {
    fn clone(&self) -> Self {
        Self { tx: self.tx.clone(), asked: self.asked.clone() }
    }
}

impl<SYS: System> Requester<SYS> {
    pub(crate) fn new(tx: &tkio::mpsc::Sender<MsgFromSys<SYS::FromSys>>, asked: &Asked<SYS::ToSys>) -> Self {
        Self { tx: tx.downgrade(), asked: Arc::downgrade(asked) }
    }

    /// `ask` sent up to the controller from a task, its answer awaited.
    pub async fn request<A: Ask<SYS>>(&self, ask: A) -> SubsysResult<A::Answer> {
        let (tx, packet, answer) = self.prepare(ask)?;
        let id = packet.id;
        if let Err(e) = tx.send(MsgFromSys::Packet(packet)).await {
            self.forget(id);
            return Err(e.into());
        }
        drop(tx);
        Self::answered(answer.await)
    }

    /// `ask` sent up from a thread outside the runtime, its answer awaited.
    pub fn request_blocking<A: Ask<SYS>>(&self, ask: A) -> SubsysResult<A::Answer> {
        let (tx, packet, answer) = self.prepare(ask)?;
        let id = packet.id;
        if let Err(e) = tx.blocking_send(MsgFromSys::Packet(packet)) {
            self.forget(id);
            return Err(e.into());
        }
        drop(tx);
        Self::answered(answer.blocking_recv())
    }

    /// The request's packet and its way up, its waiter held by the packet's id.
    #[allow(clippy::type_complexity)]
    fn prepare<A: Ask<SYS>>(&self, ask: A) -> SubsysResult<(
        tkio::mpsc::Sender<MsgFromSys<SYS::FromSys>>,
        Packet<SYS::FromSys>,
        tkio::oneshot::Receiver<SubsysResult<SYS::ToSys>>,
    )> {
        let (Some(tx), Some(asked)) = (self.tx.upgrade(), self.asked.upgrade()) else {
            return Err(SubsysError::ChannelClosed);
        };
        let packet = Packet::request(ask.into());
        let (send, answer) = tkio::oneshot::channel();
        asked.lock().expect("lock").insert(packet.id, send);
        Ok((tx, packet, answer))
    }

    /// A waiter whose request never left dropped.
    fn forget(&self, id: MsgID) {
        if let Some(asked) = self.asked.upgrade() {
            asked.lock().expect("lock").remove(&id);
        }
    }

    /// An answer taken as its type; a waiter dropped unanswered, a closed channel.
    fn answered<T: TryFrom<SYS::ToSys, Error = SubsysError>>(
        received: Result<SubsysResult<SYS::ToSys>, tkio::oneshot::error::RecvError>,
    ) -> SubsysResult<T> {
        let answer = received.map_err(|_| SubsysError::ChannelClosed)??;
        answer.try_into()
    }
}

/// `pkt` handed to the waiter it answers, or back when it answers none.
pub(crate) fn deliver<T>(asked: &Asked<T>, pkt: Packet<T>) -> Option<Packet<T>> {
    let waiter = pkt.response_to_id().and_then(|id| asked.lock().expect("lock").remove(&id));
    match waiter {
        Some(waiter) => {
            let _ = waiter.send(Ok(pkt.take_msg()));
            None
        }
        None => Some(pkt),
    }
}

/// The waiter for request `id`, which its controller left unanswered, failed.
pub(crate) fn unanswered<T>(asked: &Asked<T>, id: MsgID) {
    if let Some(waiter) = asked.lock().expect("lock").remove(&id) {
        let _ = waiter.send(Err(SubsysError::Unanswered));
    }
}
