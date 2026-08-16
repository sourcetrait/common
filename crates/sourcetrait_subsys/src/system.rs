use crate::*;
pub trait System: Sized + Send + 'static {
    const CHANNEL_SIZE: usize;
    
    type Paths: agnostic::HasAppPaths;
    type Params: Params;
    type Config: Config;
    type ToSys: cereal::Data;
    type FromSys: cereal::Data;
    type Flow: cereal::DataCopyEq;
    
    fn send_channel(&mut self, msg: MsgFromSys<Self::FromSys>) -> impl Future<Output = RunResult> {
        async {
            let inner = self.inner_mut();
            match inner.channel.tx.send(msg).await {
                Ok(_) => SUCCESS,
                Err(e) => self.unrecoverable(e, false).await,
            }
        }
    }
    
    fn send_channel_packet(&mut self, pkt: Packet<Self::FromSys>) -> impl Future<Output = RunResult> {
        self.send_channel(MsgFromSys::Packet(pkt))
    }

    fn send_channel_status_change(&mut self) -> impl Future<Output = RunResult> {
        let msg = MsgFromSys::Sub(FromSub::StatusChange(
            Packet::singular(StatusChange(self.status()))
        ));
        self.send_channel(msg)
    }
    
    /// always succeeds
    #[inline]
    fn shutdown(&mut self) -> impl Future<Output = RunResult> {
        self.stop(false)
    }
    
    /// always succeeds
    #[inline]
    fn halt(&mut self) -> impl Future<Output = RunResult> {
        self.stop(true)
    }
    
    fn status_stop(&self) -> Option<bool> {
        match self.inner().status {
            Status::NotReady(NotReady::Stop { halt }) => Some(halt),
            _ => None,
        }
    }
    
    /// always succeeds
    fn stop(&mut self, halt: bool) -> impl Future<Output = RunResult> {
        async move {
            if !self.inner().running { return SUCCESS; }
            
            let stopping = self.status_stop();
            if stopping.is_none() {
                let _ = self.inner_mut().prepare_stop(halt).await;
            }
            if halt && !stopping.is_some_and(|halting| halting) {
                let _ = self.on_stop(halt).await;
            }
            if stopping.is_none() {
                let _ = self.inner_mut().halt().await;
            }
            
            SUCCESS
        }
    }

    fn done(&mut self, result: RunResult) -> impl Future<Output = RunResult> {
        async move {
            match result {
                Ok(_) => self.shutdown().await,
                Err(_) => {
                    let _ = self.shutdown().await;
                    FAILURE
                }
            }
        }
    }
    
    fn unrecoverable<T>(&mut self, _e: impl std::error::Error, halt: bool) -> impl Future<Output = SysResult<T>> {
        async move {
            let _ = self.stop(halt).await?;
            Err(Failure)
        }
    }

    #[inline]
    fn status(&self) -> Status {
        self.inner().status
    }
    
    #[inline]
    fn is_not_ready(&self) -> bool {
        self.inner().status != Status::Ready
    }
    
    #[inline]
    fn is_paused(&self) -> bool {
        self.inner().status == Status::NotReady(NotReady::Pause)
    }
        
    #[inline]
    fn status_not_ready(&self) -> Option<NotReady> {
        match self.inner().status {
            Status::NotReady(v) => Some(v),
            _ => None
        }
    }
    
    fn handle_channel_recv(&mut self, msg: MsgToSys<Self::ToSys>) -> impl Future<Output = FlowResult<Self::Flow>> {
        async move {
            match msg {
                MsgToSys::Packet(pkt) => self.on_channel_recv(pkt).await,
                MsgToSys::Sub(ToSub::ControlRequest(pkt)) => self.handle_control_request(pkt).await,
                MsgToSys::Sub(ToSub::StatusRequest(pkt)) => self.handle_status_request(pkt).await,
                MsgToSys::Envelope => todo!("Envelopes are not implemented"),
            }
        }
    }
    
    fn handle_resume_request(&mut self, request_id: MsgID) -> impl Future<Output = FlowResult<Self::Flow>> {
        async move {
            if !self.is_paused() {
                return self.send_channel(
                    MsgFromSys::Sub(FromSub::ControlResponse(
                        Packet::response(
                            request_id,
                            ControlResponse {
                                result: SUCCESS,
                            },
                        )
                    ))
                )
                .await
                .map(|_| Flow::Continue);
            }
            
            let ready = self.on_resume().await.map_err(|_| Failure)?;
            
            let (status, result) = match ready {
                true => (Status::Ready, SUCCESS),
                false => (Status::NotReady(NotReady::Normal), FAILURE),
            };
            
            self.inner_mut().status = status; 
            self.send_channel(MsgFromSys::Sub(FromSub::ControlResponse(
                Packet::response(
                    request_id,
                    ControlResponse {
                        result, 
                    },
                )
            ))).await.map(|_| Flow::Control(Control::Resume))
        }
    }
    
    fn handle_stop_request(&mut self, request_id: MsgID, halt: bool) -> impl Future<Output = FlowResult<Self::Flow>> {
        async move {
            match self.status() {
                Status::NotReady(NotReady::Stop {halt: false}) if !halt => {
                    self.send_channel(MsgFromSys::Sub(FromSub::ControlResponse(
                        Packet::response(
                            request_id,
                            ControlResponse {
                                result: SUCCESS,
                            },
                        )
                    ))).await.map(|_| Flow::Continue)
                }
                _ => self.stop(halt).await.map(|_| Flow::Control(Control::Stop{halt}))
            }
        }
    }

    fn handle_control_request(&mut self, pkt: Packet<ControlRequest>) -> impl Future<Output = FlowResult<Self::Flow>> {
        async move {
            match pkt.msg.control {
                Control::Resume => self.handle_resume_request(pkt.id).await,
                Control::Stop {halt} => self.handle_stop_request(pkt.id, halt).await,
                Control::Drain => todo!(),
                Control::Drop => todo!(),
                Control::Refresh => todo!(),
                Control::Restart => todo!(),
            }
        }
    }
    
    fn handle_status_request(&mut self, pkt: Packet<StatusRequest>) -> impl Future<Output = FlowResult<Self::Flow>> {
        async move {
            let msg = MsgFromSys::Sub(FromSub::StatusResponse(
                pkt.respond(StatusResponse {
                    status: self.inner().status,
                })
            ));
                
            self.send_channel(msg).await?;
            Ok(Flow::Continue)
        }
    }

    fn inner(&self) -> &InnerSystem<Self>;
    
    fn inner_mut(&mut self) -> &mut InnerSystem<Self>;
    
    fn init(inner: InnerSystem<Self>, params: Self::Params) -> impl Future<Output = SubsysResult<Self>>;
        
    fn run(self) -> impl Future<Output = RunResult> + Send + 'static;

    /// Contract:
    /// - Unless stop() is called manually, will only be called if not currently
    ///   stopping with the same severity (halt)
    fn on_stop(&mut self, halt: bool) -> impl Future<Output = RunResult>;
    
    ///
    /// Contract:
    /// - Standard messages will already be handled and are safe to [unreachable!()] on:
    ///   - [ControlRequest]
    ///   - [StatusRequest]
    /// - Messages that affect how [Self::run] operates should be returned via [Self::Flow].
    fn on_channel_recv(&mut self, pkt: Packet<Self::ToSys>) -> impl Future<Output = FlowResult<Self::Flow>>;
    
    /// Returns true if ready and false if not [NotReady::Normal]. 
    /// 
    /// Contract:
    /// - Status is currently [Status::NotReady(NotReady::Pause)]
    /// - Handler will update status and notify sender.
    fn on_resume(&mut self) -> impl Future<Output = SysResult<bool>>;
}
