use std::{
  process::ExitStatus,
  sync::{
    mpsc::{self, channel, Receiver, RecvError, SendError, Sender},
    Arc, Mutex,
  },
  time::{Duration, Instant},
};

use crate::TryLockFor;

pub struct Channel<T> {
  sender: Sender<T>,
  receiver: Receiver<T>,
}

unsafe impl<T: Send> Send for Channel<T> {}
unsafe impl<T: Sync> Sync for Channel<T> {}

impl<T> Channel<T> {
  pub fn new() -> Self {
    let (tx, rx) = channel::<T>();
    Self {
      sender: tx,
      receiver: rx,
    }
  }

  pub fn send(&self, evt: T) -> Result<(), SendError<T>> {
    self.sender.send(evt)
  }

  pub fn recv(&self) -> Result<T, RecvError> {
    self.receiver.recv()
  }

  pub fn recv_timeout(&self, dur: Duration) -> Result<T, mpsc::RecvTimeoutError> {
    self.receiver.recv_timeout(dur)
  }

  // recv all possible Ts in the supplied duration
  pub fn pump(&self, dur: Duration) -> Vec<T> {
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
