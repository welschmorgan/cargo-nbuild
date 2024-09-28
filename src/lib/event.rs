use std::{
  sync::{
    mpsc::{self, channel, Receiver, RecvError, SendError, Sender},
    Arc, Mutex,
  },
  time::Duration,
};

pub enum Event {
  Dummy,
  Quit,
}

pub struct EventBusInner {
  sender: Sender<Event>,
  receiver: Receiver<Event>,
}

#[derive(Clone)]
pub struct EventBus(Arc<Mutex<EventBusInner>>);

unsafe impl Send for EventBus {}
unsafe impl Sync for EventBus {}

impl EventBus {
  pub fn new() -> Self {
    let (tx, rx) = channel::<Event>();
    Self(Arc::new(Mutex::new(EventBusInner {
      sender: tx,
      receiver: rx,
    })))
  }

  pub fn send(&self, evt: Event) -> Result<(), SendError<Event>> {
    let g = self.0.lock().unwrap();
    g.sender.send(evt)
  }

  pub fn recv(&self) -> Result<Event, RecvError> {
    let g = self.0.lock().unwrap();
    g.receiver.recv()
  }

  pub fn recv_timeout(&self, dur: Duration) -> Result<Event, mpsc::RecvTimeoutError> {
    let g = self.0.lock().unwrap();
    g.receiver.recv_timeout(dur)
  }
}
