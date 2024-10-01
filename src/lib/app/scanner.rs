use std::{
  io::{stdin, BufRead, BufReader},
  process::ExitStatus,
  sync::mpsc::Sender,
  thread::spawn,
};

use crate::{BuildEntry, BuildEvent, Debug, Origin};

pub struct Scanner {
  origin: Origin,
  tx_entries: Sender<Vec<BuildEntry>>,
  tx_events: Sender<BuildEvent>,
}

const THREADED_SCANNER: bool = false;

impl Scanner {
  pub fn new(
    origin: Origin,
    tx_entries: Sender<Vec<BuildEntry>>,
    tx_events: Sender<BuildEvent>,
  ) -> Self {
    Self {
      origin,
      tx_entries,
      tx_events,
    }
  }

  /// The stdin scanner thread
  pub fn run(self) {
    crate::dbg!("scan thread started on {:?}", self.origin);
    let _ = self.tx_events.send(BuildEvent::BuildStarted);
    Debug::log("spawned cargo process");
    let buf = BufReader::new(stdin());
    let entries = self.tx_entries.clone();
    let f = move || {
      for line in buf.lines() {
        let line = line.expect("invalid input line").replace("\x00", "");
        crate::dbg!("[stdin] {}", line);
        let _ = entries.send(vec![BuildEntry::new(line, self.origin)]);
      }
    };
    if THREADED_SCANNER {
      let thread = spawn(f);
      Debug::log("Waiting for scanner thread");
      thread
        .join()
        .expect("failed to join process scanner thread");
    } else {
      f();
    }
    let exit_status = ExitStatus::default();
    let _ = self.tx_events.send(BuildEvent::BuildFinished(exit_status));
    Debug::log(format!("Exit status: {}", exit_status));
    Debug::log("scan thread stopped");
  }
}
