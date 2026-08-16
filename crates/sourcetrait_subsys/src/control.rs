use crate::*;

/// Handles incoming messages
/// Returns None if handled, else Some(msg) if unhandled.
pub trait Handler<SYS: System>: 'static + Send {
    fn on_packet(&mut self, pkt: Packet<SYS::FromSys>)
        -> impl Future<Output = Option<Packet<SYS::FromSys>>> + 'static + Send;
}

pub struct SystemControl<SYS: System>  {
    tx: tkio::mpsc::Sender<MsgToSys<<SYS as System>::ToSys>>,
    sys_handle: tkio::task::JoinHandle<RunResult>,
    self_handle: tkio::task::JoinHandle<RunResult>,
    cancel: tkio::CancellationToken,
    requests: Arc<Mutex<FxHashMap<u64, tkio::oneshot::Sender<<SYS as System>::FromSys>>>>,
}

struct ControlTask<SYS: System, HNDLR: Handler<SYS>> {
    rx: tkio::mpsc::Receiver<MsgFromSys<<SYS as System>::FromSys>>,
    handler: HNDLR,
    requests: Arc<Mutex<FxHashMap<u64, tkio::oneshot::Sender<<SYS as System>::FromSys>>>>,
    cancel: tkio::CancellationToken,
}

impl<SYS: System, HNDLR: Handler<SYS>> ControlTask<SYS, HNDLR> {
    async fn run(mut self) -> RunResult {
            let result = loop {
            let result = tokio::select! {
                rx = self.rx.recv() => match rx {
                    None => Err(Failure),
                    Some(MsgFromSys::Packet(pkt)) if let Some(reqid) = pkt.response_to_id() => {
                        let req = self.requests.lock().expect("lock").remove(&reqid);
                        match req {
                            Some(shotx) => {
                                shotx.send(pkt.take_msg())
                                    .unwrap(); //todo
                                SUCCESS
                            },
                            None => match self.handler.on_packet(pkt).await {
                                None => SUCCESS,
                                Some(msg) => Ok(Success),
                            }
                        }
                    },
                    Some(MsgFromSys::Packet(pkt)) => {
                        match self.handler.on_packet(pkt).await {
                            Some(msg) => SUCCESS,
                            None => Ok(Success)
                        }
                    },
                    Some(msg_todo) => todo!(),
                },
            };
            
            if let Err(e) = result {
                break Err(e);
            }
        };

        result
    }
}

impl<SYS: System> SystemControl<SYS> {
    pub async fn start<HNDLR: Handler<SYS>>(
        paths: SYS::Paths,
        config: SYS::Config,
        params: SYS::Params,
        handler: HNDLR,
    ) -> SubsysResult<Self> {
        let channel = InnerSystem::<SYS>::start(
            paths,
            config,
            params,
        ).await?;

        let Channel { tx, rx, cancel, handle: sys_handle } = channel;
        let requests = Arc::new(Mutex::new(FxHashMap::default()));

        let task = ControlTask {
            rx,
            handler,
            cancel: cancel.clone(),
            requests: requests.clone(),
        };
        
        let self_handle = tokio::spawn(async move { task.run().await });
        
        let this = Self {
            tx,
            cancel,
            sys_handle,
            self_handle,
            requests,
        };
        
        Ok(this)
    }
    
    pub async fn send_sysmsg(&self, sysmsg: MsgToSys<SYS::ToSys>) -> SubsysResult<()> {
        self.tx.send(sysmsg).await?;
        Ok(())
    }
    
    pub async fn send_packet(&self, pkt: Packet<SYS::ToSys>) -> SubsysResult<()> {
        self.tx.send(MsgToSys::Packet(pkt)).await?;
        Ok(())
    }
    
    pub async fn request<T: Request<SYS>>(&self, req: T) -> SubsysResult<T::ResponseType>
    where
        <T as Request<SYS>>::ResponseType: TryFrom<<SYS as System>::FromSys, Error = SubsysError>
    {
        let packet = Packet::request(req.into());
        let reqid = packet.id;
        let msg = MsgToSys::Packet(packet);
        let (send, recv) = tkio::oneshot::channel();
        self.requests
            .lock().expect("lock")
            .insert(reqid, send);
        self.tx.send(msg).await?;
        let response: T::ResponseType = recv.await
            .map_err(|_| SubsysError::Fatal)?
            .try_into()?;

        Ok(response)
    }
}

pub trait Request<SYS: System>: Into<SYS::ToSys> {
    type ResponseType: TryFrom<SYS::FromSys, Error = SubsysError>;
}
