///! Defines a subsystem that will, upon request, either Add or Multiply a number
///! by a specified operand and an internally accumulated number. The internal
///! number is initialized by configuration.
use sourcetrait_subsys::{self as subsys, prelude::* };
use sourcetrait_cereal_macro as cereal;
use sourcetrait_agnostic::{ self as agnostic, prelude::* };
use sourcetrait_tomlx::{ self as tomlx, prelude::* };
use std::{
    path::PathBuf,
};

#[tokio::main]
async fn main() { basic().await }

#[tokio::test]
async fn test_basic() { basic().await }


async fn basic() {
    let paths = ExampleSysPaths::default();
    let config = ExampleSysConfig::default();
    let params = ExampleSysParams { op: Operation::Mul };

    struct Handler;
    impl subsys::Handler<ExampleSys> for Handler {
        fn on_packet(&mut self, msg: subsys::Packet<FromExampleSys>) -> impl Future<Output = Option<subsys::Packet<FromExampleSys>>> + 'static + Send {
            async move {
                None
            }
        }
    }
    
    let mut sys: subsys::SystemControl<ExampleSys> = subsys::SystemControl::start(
        paths,
        config,
        params,
        Handler,
    ).await.unwrap();
    
    let request = MathRequest {
        operand: 2.,
    };
    /* What gets sent here:
    let expected_packet = subsys::Packet {
        id: 1,
        nature: subsys::PacketNature::Request,
        msg: ToExampleSys::MathRequest(
            MathRequest {
                operand: 2.,
            }
        ),
    };
    */
    
    let response = sys.request(request).await.expect("response");
    let expected_response = MathResponse {
        result: Ok(2.),
    };
    assert_eq!(expected_response, response);
    
    let request = MathRequest {
        operand: 3.,
    };
    /* What gets sent here:
    let expected_packet = subsys::Packet {
        id: 3,
        nature: subsys::PacketNature::Request,
        msg: ToExampleSys::MathRequest(
            MathRequest {
                operand: 3.,
            }
        ),
    };
    */
    
    let expected_response = MathResponse {
        result: Ok(6.),
    };
    let response = sys.request(request).await.unwrap();
    assert_eq!(expected_response, response);
}

pub struct ExampleSys {
    inner: subsys::InnerSystem<Self>,
    op: Operation,
    num: f64,
}

// SYSTEM CHANNEL AND MESSAGES

#[cereal::derived(Copy, Eq, Data)]
pub enum Operation {
    Add,
    Mul,
}

#[cereal::derived(Copy, Eq, Data)]
pub enum MathFail {
    Error,
}

pub type MathResult = Result<f64, MathFail>;

#[cereal::derived(Copy, Data)]
pub struct MathRequest {
    operand: f64,
}

#[cereal::derived(Copy, Data)]
pub enum ToExampleSys {
    MathRequest(MathRequest),
}

#[cereal::derived(Copy, Data)]
pub struct MathResponse {
    result: MathResult,
}

impl subsys::Request<ExampleSys> for MathRequest { type ResponseType = MathResponse; }
impl From<MathRequest> for ToExampleSys {
    fn from(v: MathRequest) -> Self { ToExampleSys::MathRequest(v) }
}
impl TryFrom<FromExampleSys> for MathResponse {
    type Error = subsys::SubsysError;

    fn try_from(v: FromExampleSys) -> Result<Self, Self::Error> {
        match v {
            FromExampleSys::MathResponse(v) => Ok(v),
            _ => Err(subsys::SubsysError::ResponseType),
        }
    }
}

#[cereal::derived(Data)]
pub enum FromExampleSys {
    MathResponse(MathResponse),
}

#[cereal::derived(Eq, Data)]
pub struct ExampleSysParams {
    op: Operation,
}

impl subsys::Params for ExampleSysParams {}

impl subsys::System for ExampleSys {
    const CHANNEL_SIZE: usize = 100;
    
    type Paths = ExampleSysPaths;
    type Params = ExampleSysParams;
    type Config = ExampleSysConfig;
    type ToSys = ToExampleSys;
    type FromSys = FromExampleSys;
    type Flow = subsys::StdFlow;

    fn inner(&self) -> &subsys::InnerSystem<Self> { &self.inner }
    fn inner_mut(&mut self) -> &mut subsys::InnerSystem<Self> { &mut self.inner }
    
    async fn init(inner: subsys::InnerSystem<Self>, params: Self::Params) -> subsys::SubsysResult<Self> {
        Ok(Self {
            op: params.op,
            num: 1.,
            inner,
        })
    }
    
    async fn run(mut self) -> subsys::RunResult {
        let result = loop {
            let result = tokio::select! {
                rx = self.inner.channel.rx.recv() => match rx {
                    Some(msg) => self.handle_channel_recv(msg).await,
                    None => Err(subsys::Failure),
                },
            };
            
            if let Err(e) = result {
                break Err(e);
            }
        };
        
        self.done(result).await
    }
    
    async fn on_channel_recv(&mut self, pkt: subsys::Packet<ToExampleSys>) -> subsys::FlowResult<Self::Flow> {
        let (id, nature, msg) = pkt.into_tuple();
        match (nature, msg) {
            (subsys::PacketNature::Request, ToExampleSys::MathRequest(req))
                => self.on_math_request(subsys::Packet::new(id, nature, req)).await,
            _ => todo!(),
        }
    }

    async fn on_stop(&mut self, _halt: bool) -> subsys::RunResult {
        subsys::SUCCESS
    }

    async fn on_resume(&mut self) -> subsys::SysResult<bool> {
        Ok(true)
    }
}

impl ExampleSys {
    async fn on_math_request(&mut self, req: subsys::Packet<MathRequest>) -> subsys::FlowResult<subsys::StdFlow> {
        self.num = match self.op {
            Operation::Add => req.msg.operand + self.num,
            Operation::Mul => req.msg.operand * self.num,
        };
        
        self.send_channel_packet(req.respond(FromExampleSys::MathResponse(
            MathResponse {
                result: Ok(self.num),
            }
        ))).await?;
        
        Ok(subsys::Flow::Continue)
    }
}

// SYSTEM PATHS

#[derive(Clone)]
pub struct ExampleSysPaths(agnostic::AppPathRouter);

impl Default for ExampleSysPaths {
    fn default() -> Self {
        Self(agnostic::DefaultAppPaths::Default(
            "sourcetrait/subsys/examples/basic"
        ).into())
    }
}

impl HasAppPaths for ExampleSysPaths {
    fn app_paths(&self) -> &impl AppPaths {
        &self.0
    }
}

impl ExampleSysPaths {
    pub const CONFIG_TOML: &'static str = "config.toml";
    
    pub fn config_toml(&self) -> PathBuf {
        self.config_dir().join(Self::CONFIG_TOML)
    }
}

// SYSTEM CONFIG 

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExampleSysConfig {
}

impl ExampleSysConfig {
    pub fn read(paths: &ExampleSysPaths) -> subsys::SubsysResult<Self> {
        let config_path = paths.config_toml(); 
        ExampleSysConfig::from_toml_file(&config_path)
            .map_err(|e| subsys::SubsysError::into_io(e))
    }
}

impl subsys::Config for ExampleSysConfig {}
impl tomlx::FromToml for ExampleSysConfig {}


