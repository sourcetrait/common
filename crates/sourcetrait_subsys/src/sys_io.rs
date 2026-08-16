use crate::*;

pub enum SysIo {
    Stdio,
    LineChannel(LineChannel),
}

impl SysIo {
    pub fn new_channel(size: usize) -> (Self, LineChannel) {
        let (left_tx, left_rx) = tkio::mpsc::channel(size);
        let (right_tx, right_rx) = tkio::mpsc::channel(size);
        
        (
            Self::LineChannel(LineChannel {
                output: left_tx,
                input: left_rx,
            }),
            LineChannel {
                output: right_tx,
                input: right_rx,
            },
        )
    }
    
    pub fn is_stdio(&self) -> bool {
        matches!(self, Self::Stdio)
    }
    
    pub async fn read_line(&mut self) -> SubsysResult<String> {
        match self {
            Self::Stdio => {
                let handle = tokio::task::spawn_blocking(move || -> io::Result<String> {
                    let mut s = String::new();
                    stdin().read_line(&mut s).map(|_| s)
                });
                
                let s = handle.await??;
                Ok(s)
            },
            Self::LineChannel(channel) => {
                tokio::select! {
                    rx = channel.input.recv() => match rx {
                        Some(line) => Ok(line),
                        None => SubsysError::err_channel_closed(),
                    }
                }
            }
        }
    }
}

pub struct LineChannel {
    pub output: tkio::mpsc::Sender<String>,
    pub input: tkio::mpsc::Receiver<String>,
}
