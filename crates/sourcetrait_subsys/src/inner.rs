use crate::*;

pub struct InnerSystem<SYS: System> {
    pub paths: SYS::Paths,
    pub config: SYS::Config,
    pub channel: InternalChannel<MsgFromSys<<SYS as System>::FromSys>, MsgToSys<<SYS as System>::ToSys>>,
    pub running: bool,
    pub status: Status,
    /// The answers the system awaits from its controller, by request id.
    pub(crate) asked: Asked<<SYS as System>::ToSys>,
}

impl<SYS: System> InnerSystem<SYS> {
    pub async fn start(
        paths: SYS::Paths,
        config: SYS::Config,
        params: SYS::Params,
    ) -> SubsysResult<Channel<MsgToSys<SYS::ToSys>, MsgFromSys<SYS::FromSys>>> {
        let (channel, external_channel) = InternalChannel::new_pair(SYS::CHANNEL_SIZE);
        let cancel = channel.cancel.clone();
        let this = Self {
            paths,
            config,
            channel,
            running: true,
            status: Status::NotReady(NotReady::Normal),
            asked: Arc::new(Mutex::new(FxHashMap::default())),
        };
        let sys = SYS::init(this, params).await?;
        let handle = tokio::task::spawn(async move { sys.run().await });

        Ok(Channel::new(external_channel, cancel, handle))
    }
    
    pub async fn prepare_stop(&mut self, halt: bool) -> RunResult {
        if !self.running { return SUCCESS }
        
        self.status = Status::NotReady(NotReady::Stop { halt });
        
        if !self.channel.tx.is_closed() && !halt {
            let msg = MsgFromSys::Sub(
                FromSub::StatusChange(Packet::singular(StatusChange(self.status))
            ));
            let _ = self.channel.tx.send(msg).await;
        }
        
        SUCCESS
    }

    /// A handle the system asks its controller through, from a task or a thread.
    pub fn requester(&self) -> Requester<SYS> {
        Requester::new(&self.channel.tx, &self.asked)
    }

    pub async fn halt(&mut self) -> RunResult {
        if !self.running { return SUCCESS }
        
        self.status = Status::NotReady(NotReady::Stop { halt: true });
        self.asked.lock().expect("lock").clear();
        self.channel.cancel.cancel();
        
        if !self.channel.rx.is_closed() {
            self.channel.rx.close();
        }
        
        self.running = false;
        SUCCESS
    }
}

