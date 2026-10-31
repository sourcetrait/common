//! A system's requests of its controller: asked from a task and from a thread,
//! each answer reaching its own waiter, none hanging unanswered, stopped, or ended.
use sourcetrait_subsys as subsys;
use sourcetrait_cereal_macro as cereal;
use sourcetrait_agnostic::{self as agnostic, prelude::*};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

/// How long a test waits on an answer before calling it a hang.
const HANG: Duration = Duration::from_secs(5);

/// The question the controller never answers in time, for the stop's test.
const STALLED: u64 = 999;

/// What each failed ask came to, by its number, written by the system's task.
static OUTCOMES: Mutex<BTreeMap<u64, String>> = Mutex::new(BTreeMap::new());

/// The requester a system handed out, kept past the system's end.
static KEPT: Mutex<Option<subsys::Requester<AskSys>>> = Mutex::new(None);

#[tokio::test]
async fn a_system_asks_from_a_task_and_hears_its_answer() {
    let control = start().await;
    let done = control.request(Go { n: 3, blocking: false }).await.expect("go");
    assert_eq!(done, Done { n: 3, value: Some(30), unanswered: false });
    control.stop(true).await.expect("stop");
}

#[tokio::test]
async fn a_system_asks_from_a_blocking_thread() {
    let control = start().await;
    let done = tokio::time::timeout(HANG, control.request(Go { n: 4, blocking: true }))
        .await
        .expect("the blocking ask ends rather than hanging")
        .expect("go");
    assert_eq!(done, Done { n: 4, value: Some(40), unanswered: false });
    control.stop(true).await.expect("stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn each_answer_reaches_its_own_waiter() {
    let control = std::sync::Arc::new(start().await);
    let asks: Vec<_> = (1..=8u64)
        .map(|n| {
            let control = control.clone();
            tokio::spawn(async move { control.request(Go { n, blocking: n % 2 == 0 }).await })
        })
        .collect();
    for (n, ask) in (1..=8u64).zip(asks) {
        let done = tokio::time::timeout(HANG, ask).await.expect("no hang").expect("joined").expect("go");
        assert_eq!(done, Done { n, value: Some(n * 10), unanswered: false });
    }
    let control = std::sync::Arc::into_inner(control).expect("every ask let go of the control");
    control.stop(true).await.expect("stop");
}

#[tokio::test]
async fn an_unanswered_request_fails_rather_than_hangs() {
    let control = start().await;
    let done = tokio::time::timeout(HANG, control.request(Go { n: 0, blocking: false }))
        .await
        .expect("the unanswered ask ends rather than hanging")
        .expect("go");
    assert_eq!(done, Done { n: 0, value: None, unanswered: true });
    control.stop(true).await.expect("stop");
}

#[tokio::test]
async fn a_stop_fails_the_requests_still_waiting() {
    let control = start().await;
    control.send_packet(subsys::Packet::request(ToAskSys::Go(Go { n: STALLED, blocking: false }))).await.expect("send");
    let asked = async {
        while !Asker::waiting() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    };
    tokio::time::timeout(HANG, asked).await.expect("the stalled ask is waiting");
    control.stop(true).await.expect("stop");
    let failed = async {
        loop {
            if let Some(outcome) = OUTCOMES.lock().expect("lock").get(&STALLED).cloned() {
                break outcome;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    };
    let outcome = tokio::time::timeout(HANG, failed).await.expect("the stalled ask ends");
    assert_eq!(outcome, "Channel closed");
}

#[tokio::test]
async fn a_request_after_the_system_ends_fails_at_once() {
    let control = start().await;
    control.request(Keep).await.expect("keep");
    control.stop(true).await.expect("stop");
    let requester = KEPT.lock().expect("lock").take().expect("the system handed its requester out");
    let asked = tokio::time::timeout(HANG, requester.request(Question { n: 1 }))
        .await
        .expect("the ask ends rather than hanging");
    assert!(matches!(asked, Err(subsys::SubsysError::ChannelClosed)), "{asked:?}");
    let blocking = tokio::task::spawn_blocking(move || requester.request_blocking(Question { n: 2 }));
    let asked = tokio::time::timeout(HANG, blocking).await.expect("no hang").expect("joined");
    assert!(matches!(asked, Err(subsys::SubsysError::ChannelClosed)), "{asked:?}");
}

async fn start() -> subsys::SystemControl<AskSys> {
    subsys::SystemControl::start(AskSysPaths::default(), AskSysConfig::default(), AskSysParams {}, Asker)
        .await
        .expect("start")
}

/// The controller: each question answered with ten times its number, after a
/// delay that scrambles the order; zero left unanswered; the stalled one held.
struct Asker;

impl Asker {
    /// Whether the stalled question has reached the controller.
    fn waiting() -> bool {
        STALLED_ASKED.load(std::sync::atomic::Ordering::SeqCst)
    }
}

static STALLED_ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl subsys::Handler<AskSys> for Asker {
    fn on_packet(&mut self, pkt: subsys::Packet<FromAskSys>) -> impl Future<Output = Option<subsys::Packet<FromAskSys>>> + 'static + Send {
        async move { Some(pkt) }
    }

    fn on_request(&mut self, pkt: subsys::Packet<FromAskSys>) -> impl Future<Output = Option<ToAskSys>> + 'static + Send {
        async move {
            let FromAskSys::Question(Question { n }) = pkt.take_msg() else { return None };
            match n {
                0 => None,
                STALLED => {
                    STALLED_ASKED.store(true, std::sync::atomic::Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    None
                }
                n => {
                    tokio::time::sleep(Duration::from_millis((9 - n % 9) * 3)).await;
                    Some(ToAskSys::Answer(Answer { value: n * 10 }))
                }
            }
        }
    }
}

pub struct AskSys {
    inner: subsys::InnerSystem<Self>,
}

/// The controller's request: ask your controller question `n`, from a task or
/// from a blocking thread, and say what came back.
#[cereal::derived(Copy, Eq, Data)]
pub struct Go {
    n: u64,
    blocking: bool,
}

#[cereal::derived(Copy, Eq, Data)]
pub struct Done {
    n: u64,
    value: Option<u64>,
    unanswered: bool,
}

/// The controller's request: hand your requester out, to be asked by after your end.
#[cereal::derived(Copy, Eq, Data)]
pub struct Keep;

#[cereal::derived(Copy, Eq, Data)]
pub struct Kept;

/// The system's request of its controller.
#[cereal::derived(Copy, Eq, Data)]
pub struct Question {
    n: u64,
}

#[cereal::derived(Copy, Eq, Data)]
pub struct Answer {
    value: u64,
}

#[cereal::derived(Copy, Eq, Data)]
pub enum ToAskSys {
    Go(Go),
    Keep(Keep),
    Answer(Answer),
}

#[cereal::derived(Copy, Eq, Data)]
pub enum FromAskSys {
    Done(Done),
    Kept(Kept),
    Question(Question),
}

impl subsys::Request<AskSys> for Go { type ResponseType = Done; }
impl subsys::Request<AskSys> for Keep { type ResponseType = Kept; }
impl subsys::Ask<AskSys> for Question { type Answer = Answer; }

impl From<Go> for ToAskSys {
    fn from(v: Go) -> Self { ToAskSys::Go(v) }
}

impl From<Keep> for ToAskSys {
    fn from(v: Keep) -> Self { ToAskSys::Keep(v) }
}

impl From<Question> for FromAskSys {
    fn from(v: Question) -> Self { FromAskSys::Question(v) }
}

impl TryFrom<FromAskSys> for Done {
    type Error = subsys::SubsysError;

    fn try_from(v: FromAskSys) -> Result<Self, Self::Error> {
        match v {
            FromAskSys::Done(v) => Ok(v),
            _ => Err(subsys::SubsysError::ResponseType),
        }
    }
}

impl TryFrom<FromAskSys> for Kept {
    type Error = subsys::SubsysError;

    fn try_from(v: FromAskSys) -> Result<Self, Self::Error> {
        match v {
            FromAskSys::Kept(v) => Ok(v),
            _ => Err(subsys::SubsysError::ResponseType),
        }
    }
}

impl TryFrom<ToAskSys> for Answer {
    type Error = subsys::SubsysError;

    fn try_from(v: ToAskSys) -> Result<Self, Self::Error> {
        match v {
            ToAskSys::Answer(v) => Ok(v),
            _ => Err(subsys::SubsysError::ResponseType),
        }
    }
}

#[cereal::derived(Eq, Data)]
pub struct AskSysParams {}

impl subsys::Params for AskSysParams {}

/// What one ask came to, as the controller's request reports it.
fn done(n: u64, answer: subsys::SubsysResult<Answer>) -> Done {
    match answer {
        Ok(Answer { value }) => Done { n, value: Some(value), unanswered: false },
        Err(subsys::SubsysError::Unanswered) => Done { n, value: None, unanswered: true },
        Err(e) => {
            OUTCOMES.lock().expect("lock").insert(n, e.to_string());
            Done { n, value: None, unanswered: false }
        }
    }
}

impl subsys::System for AskSys {
    const CHANNEL_SIZE: usize = 16;

    type Paths = AskSysPaths;
    type Params = AskSysParams;
    type Config = AskSysConfig;
    type ToSys = ToAskSys;
    type FromSys = FromAskSys;
    type Flow = subsys::StdFlow;

    fn inner(&self) -> &subsys::InnerSystem<Self> { &self.inner }
    fn inner_mut(&mut self) -> &mut subsys::InnerSystem<Self> { &mut self.inner }

    async fn init(inner: subsys::InnerSystem<Self>, _params: Self::Params) -> subsys::SubsysResult<Self> {
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
                Ok(_) => {}
                Err(e) => break Err(e),
            }
        };
        self.done(result).await
    }

    async fn on_channel_recv(&mut self, pkt: subsys::Packet<ToAskSys>) -> subsys::FlowResult<Self::Flow> {
        let (n, blocking) = match *pkt.msg() {
            ToAskSys::Go(Go { n, blocking }) => (n, blocking),
            ToAskSys::Keep(_) => {
                *KEPT.lock().expect("lock") = Some(self.inner.requester());
                self.send_channel_packet(pkt.respond(FromAskSys::Kept(Kept))).await?;
                return Ok(subsys::Flow::Continue);
            }
            ToAskSys::Answer(_) => return Ok(subsys::Flow::Continue),
        };
        let requester = self.inner.requester();
        let tx = self.inner.channel.tx.clone();
        if blocking {
            std::thread::spawn(move || {
                let answer = requester.request_blocking(Question { n });
                let _ = tx.blocking_send(subsys::MsgFromSys::Packet(pkt.respond(FromAskSys::Done(done(n, answer)))));
            });
        } else {
            tokio::spawn(async move {
                let answer = requester.request(Question { n }).await;
                let _ = tx.send(subsys::MsgFromSys::Packet(pkt.respond(FromAskSys::Done(done(n, answer))))).await;
            });
        }
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
pub struct AskSysPaths(agnostic::AppPathRouter);

impl Default for AskSysPaths {
    fn default() -> Self {
        Self(agnostic::DefaultAppPaths::Default("sourcetrait/subsys/tests/request").into())
    }
}

impl HasAppPaths for AskSysPaths {
    fn app_paths(&self) -> &impl AppPaths {
        &self.0
    }
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AskSysConfig {}

impl subsys::Config for AskSysConfig {}
