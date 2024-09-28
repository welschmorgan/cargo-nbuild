use std::{
  process::ExitStatus,
  sync::{
    mpsc::{self, channel, Receiver, RecvError, SendError, Sender},
    Arc, Mutex,
  },
  time::{Duration, Instant},
};

use crate::TryLockFor;

pub enum Event {
  // Cargo build wrote something on stdout
  OutputLine(String),
  // Cargo build wrote something on stderr
  ErrorLine(String),
  // Cargo build finished
  FinishedExecution(ExitStatus),
  // User pressed 'q'
  UserQuitRequest,
}

pub struct EventBusInner {
  sender: Sender<Event>,
  receiver: Receiver<Event>,
}

#[derive(Clone)]
pub struct EventBus(Arc<EventBusInner>);

unsafe impl Send for EventBus {}
unsafe impl Sync for EventBus {}

impl EventBus {
  pub fn new() -> Self {
    let (tx, rx) = channel::<Event>();
    Self(Arc::new(EventBusInner {
      sender: tx,
      receiver: rx,
    }))
  }

  pub fn send(&self, evt: Event) -> Result<(), SendError<Event>> {
    self.0.sender.send(evt)
  }

  pub fn recv(&self) -> Result<Event, RecvError> {
    self.0.receiver.recv()
  }

  pub fn recv_timeout(&self, dur: Duration) -> Result<Event, mpsc::RecvTimeoutError> {
    self.0.receiver.recv_timeout(dur)
  }

  // recv all possible events in the supplied duration
  pub fn pump(&self, dur: Duration) -> Vec<Event> {
    let mut ret = vec![];
    let start = Instant::now();
    let end = start + dur;
    while Instant::now() < end {
      if let Ok(evt) = self.recv_timeout(dur) {
        ret.push(evt);
      }
    }
    ret
  }
}
