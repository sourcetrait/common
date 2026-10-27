use crate::*;

/// Handles incoming messages
/// Returns None if handled, else Some(msg) if unhandled.
pub trait Handler<SYS: System>: 'static + Send {
    fn on_packet(&mut self, pkt: Packet<SYS::FromSys>)
        -> impl Future<Output = Option<Packet<SYS::FromSys>>> + 'static + Send;

    /// Handles a message of the system's framework: a status change or a response.
    fn on_sub(&mut self, _sub: FromSub) -> impl Future<Output = ()> + Send {
        async {}
    }
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
                _ = self.cancel.cancelled() => break SUCCESS,
                rx = self.rx.recv() => match rx {
                    None => Err(Failure),
                    Some(MsgFromSys::Packet(pkt)) => {
                        let waiter = pkt.response_to_id()
                            .and_then(|reqid| self.requests.lock().expect("lock").remove(&reqid));
                        match waiter {
                            Some(shotx) => {
                                let _ = shotx.send(pkt.take_msg());
                            },
                            None => {
                                let _ = self.handler.on_packet(pkt).await;
                            },
                        }
                        SUCCESS
                    },
                    Some(MsgFromSys::Sub(sub)) => {
                        self.handler.on_sub(sub).await;
                        SUCCESS
                    },
                    Some(MsgFromSys::Envelope) => SUCCESS,
                },
            };

            if let Err(e) = result {
                break Err(e);
            }
        };

        self.requests.lock().expect("lock").clear();
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
        if let Err(e) = self.tx.send(msg).await {
            self.requests.lock().expect("lock").remove(&reqid);
            return Err(e.into());
        }
        let response: T::ResponseType = recv.await
            .map_err(|_| SubsysError::Fatal)?
            .try_into()?;

        Ok(response)
    }

    /// Stops the system, then waits for it and the control task to end.
    pub async fn stop(self, halt: bool) -> SubsysResult<()> {
        let request = Packet::request(ControlRequest { control: Control::Stop { halt } });
        let _ = self.tx.send(MsgToSys::Sub(ToSub::ControlRequest(request))).await;
        let system = self.sys_handle.await;
        self.cancel.cancel();
        let _control = self.self_handle.await?;
        match system? {
            Ok(_) => Ok(()),
            Err(_) => Err(SubsysError::TaskFail),
        }
    }
}

pub trait Request<SYS: System>: Into<SYS::ToSys> {
    type ResponseType: TryFrom<SYS::FromSys, Error = SubsysError>;
}
