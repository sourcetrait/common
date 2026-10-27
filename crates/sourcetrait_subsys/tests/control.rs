//! The control task's lifecycle: late responses, status changes, init, and ends.
use sourcetrait_subsys as subsys;
use sourcetrait_cereal_macro as cereal;
use sourcetrait_agnostic::{self as agnostic, prelude::*};
use std::time::Duration;

/// How long a test waits on an answer before calling it a hang.
const HANG: Duration = Duration::from_secs(5);

#[tokio::test]
async fn a_late_response_leaves_the_control_task_serving() {
    let control = start(false).await.expect("start");
    let early = tokio::time::timeout(
        Duration::from_millis(50),
        control.request(EchoRequest { value: 1, delay_ms: 300 }),
    ).await;
    assert!(early.is_err(), "the first request must give up before its answer");
    let late = tokio::time::timeout(HANG, control.request(EchoRequest { value: 2, delay_ms: 0 }))
        .await
        .expect("the second request answers rather than hanging")
        .expect("the second request succeeds");
    assert_eq!(late.value, 2);
    control.stop(false).await.expect("stop");
}

#[tokio::test]
async fn a_stop_reaches_the_control_task_as_a_status_change() {
    let control = start(false).await.expect("start");
    let answer = control.request(EchoRequest { value: 3, delay_ms: 0 }).await.expect("echo");
    assert_eq!(answer.value, 3);
    tokio::time::timeout(HANG, control.stop(false))
        .await
        .expect("the stop ends rather than hanging")
        .expect("the system and its control task end cleanly");
}

#[tokio::test]
async fn a_halt_ends_the_system_and_its_control_task() {
    let control = start(false).await.expect("start");
    tokio::time::timeout(HANG, control.stop(true))
        .await
        .expect("the halt ends rather than hanging")
        .expect("the system and its control task end cleanly");
}

#[tokio::test]
async fn a_failed_init_is_an_error() {
    assert!(start(true).await.is_err());
}

#[tokio::test]
async fn a_request_to_a_stopping_system_fails_rather_than_hanging() {
    let control = start(false).await.expect("start");
    let stop = subsys::Packet::request(subsys::ControlRequest {
        control: subsys::Control::Stop { halt: false },
    });
    control
        .send_sysmsg(subsys::MsgToSys::Sub(subsys::ToSub::ControlRequest(stop)))
        .await
        .expect("send the stop");
    let answer = tokio::time::timeout(HANG, control.request(EchoRequest { value: 4, delay_ms: 0 }))
        .await
        .expect("the request ends rather than hanging");
    assert!(answer.is_err(), "a stopped system answers no request");
}

async fn start(fail_init: bool) -> subsys::SubsysResult<subsys::SystemControl<EchoSys>> {
    subsys::SystemControl::start(
        EchoSysPaths::default(),
        EchoSysConfig::default(),
        EchoSysParams { fail_init },
        Handler,
    ).await
}

struct Handler;

impl subsys::Handler<EchoSys> for Handler {
    fn on_packet(&mut self, pkt: subsys::Packet<FromEchoSys>) -> impl Future<Output = Option<subsys::Packet<FromEchoSys>>> + 'static + Send {
        async move { Some(pkt) }
    }
}

pub struct EchoSys {
    inner: subsys::InnerSystem<Self>,
}

#[cereal::derived(Copy, Eq, Data)]
pub struct EchoRequest {
    value: u64,
    delay_ms: u64,
}

#[cereal::derived(Copy, Eq, Data)]
pub struct EchoResponse {
    value: u64,
}

#[cereal::derived(Copy, Eq, Data)]
pub enum ToEchoSys {
    Echo(EchoRequest),
}

#[cereal::derived(Copy, Eq, Data)]
pub enum FromEchoSys {
    Echo(EchoResponse),
}

impl subsys::Request<EchoSys> for EchoRequest { type ResponseType = EchoResponse; }

impl From<EchoRequest> for ToEchoSys {
    fn from(v: EchoRequest) -> Self { ToEchoSys::Echo(v) }
}

impl TryFrom<FromEchoSys> for EchoResponse {
    type Error = subsys::SubsysError;

    fn try_from(v: FromEchoSys) -> Result<Self, Self::Error> {
        match v {
            FromEchoSys::Echo(v) => Ok(v),
        }
    }
}

#[cereal::derived(Eq, Data)]
pub struct EchoSysParams {
    fail_init: bool,
}

impl subsys::Params for EchoSysParams {}

impl subsys::System for EchoSys {
    const CHANNEL_SIZE: usize = 16;

    type Paths = EchoSysPaths;
    type Params = EchoSysParams;
    type Config = EchoSysConfig;
    type ToSys = ToEchoSys;
    type FromSys = FromEchoSys;
    type Flow = subsys::StdFlow;

    fn inner(&self) -> &subsys::InnerSystem<Self> { &self.inner }
    fn inner_mut(&mut self) -> &mut subsys::InnerSystem<Self> { &mut self.inner }

    async fn init(inner: subsys::InnerSystem<Self>, params: Self::Params) -> subsys::SubsysResult<Self> {
        if params.fail_init {
            return Err(subsys::SubsysError::Fatal);
        }
        Ok(Self { inner })
    }

    async fn run(mut self) -> subsys::RunResult {
        let result = loop {
            let flow = match self.inner.channel.rx.recv().await {
                Some(msg) => self.handle_channel_recv(msg).await,
                None => break subsys::SUCCESS,
            };
            match flow {
                Ok(subsys::Flow::Control(subsys::Control::Stop { .. })) => break subsys::SUCCESS,
                Ok(_) => {},
                Err(e) => break Err(e),
            }
        };

        self.done(result).await
    }

    async fn on_channel_recv(&mut self, pkt: subsys::Packet<ToEchoSys>) -> subsys::FlowResult<Self::Flow> {
        let ToEchoSys::Echo(req) = *pkt.msg();
        tokio::time::sleep(Duration::from_millis(req.delay_ms)).await;
        self.send_channel_packet(pkt.respond(FromEchoSys::Echo(EchoResponse { value: req.value })))
            .await?;
        Ok(subsys::Flow::Continue)
    }

    async fn on_stop(&mut self, _halt: bool) -> subsys::RunResult {
        subsys::SUCCESS
    }

    async fn on_resume(&mut self) -> subsys::SysResult<bool> {
        Ok(true)
    }
}

#[derive(Clone)]
pub struct EchoSysPaths(agnostic::AppPathRouter);

impl Default for EchoSysPaths {
    fn default() -> Self {
        Self(agnostic::DefaultAppPaths::Default("sourcetrait/subsys/tests/control").into())
    }
}

impl HasAppPaths for EchoSysPaths {
    fn app_paths(&self) -> &impl AppPaths {
        &self.0
    }
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EchoSysConfig {}

impl subsys::Config for EchoSysConfig {}
