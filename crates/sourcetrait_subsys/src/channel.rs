use crate::*;

pub struct Channel<TX, RX> {
    pub tx: tkio::mpsc::Sender<TX>,
    pub rx: tkio::mpsc::Receiver<RX>,
    pub cancel: tkio::CancellationToken,
    pub handle: tkio::task::JoinHandle<RunResult>,
}

pub struct InternalChannel<TX, RX> {
    pub tx: tkio::mpsc::Sender<TX>,
    pub rx: tkio::mpsc::Receiver<RX>,
    pub cancel: tkio::CancellationToken,
}

impl<TX,RX> InternalChannel<TX,RX> {
    pub fn new_pair(size: usize) -> (Self, (tkio::mpsc::Sender<RX>, tkio::mpsc::Receiver<TX>)) {
        let (left_tx, right_rx) = tkio::mpsc::channel(size);
        let (right_tx, left_rx) = tkio::mpsc::channel(size);
        let cancel = tkio::CancellationToken::new();
        
        ( Self { tx: left_tx, rx: left_rx, cancel }, (right_tx, right_rx) )
    }
    
    pub async fn send(&self, msg: TX) -> SubsysResult<()> {
        self.tx.send(msg).await?;
        Ok(())
    }
}

impl<TX,RX> Channel<TX, RX> {
    pub fn new(tuple: (tkio::mpsc::Sender<TX>, tkio::mpsc::Receiver<RX>), handle: tkio::task::JoinHandle<RunResult>) -> Self {
        let cancel = tkio::CancellationToken::new();
        Self { tx: tuple.0, rx: tuple.1, cancel, handle }
    }
    
    pub async fn send(&self, msg: TX) -> SubsysResult<()> {
        self.tx.send(msg).await?;
        Ok(())
    }
    
    pub async fn join(self) -> SubsysResult<()> {
        match self.handle.await? {
            Ok(_) => Ok(()),
            Err(_) => Err(SubsysError::TaskFail)
        }
    }
}

