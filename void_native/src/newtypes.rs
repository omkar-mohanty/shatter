use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use void_core::{IEvent, IEventReceiver, IEventSender, Result};

#[derive(Hash, Clone, Copy, Eq, PartialEq)]
pub enum NativeEvent {
    Render,
}

impl IEvent for NativeEvent {}

pub struct MpscReceiver<T>(pub UnboundedReceiver<T>);
#[derive(Clone)]
pub struct MpscSender<T>(pub UnboundedSender<T>);

pub fn create_mpsc_channel<T>() -> (MpscSender<T>, MpscReceiver<T>) {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<T>();
    (MpscSender(sender), MpscReceiver(receiver))
}

impl<T: IEvent + 'static + Send> IEventSender<T> for MpscSender<T> {
    async fn send(&self, cmd: T) -> Result<()> {
        self.0.send(cmd)?;
        Ok(())
    }

    fn send_blocking(&self, cmd: T) -> Result<()> {
        self.0.send(cmd)?;
        Ok(())
    }
}

impl<T: IEvent + 'static + Send> IEventReceiver<T> for MpscReceiver<T> {
    async fn recv(&mut self) -> Option<T> {
        self.0.recv().await
    }

    fn recv_blockding(&mut self) -> Option<T> {
        self.0.blocking_recv()
    }
}
