use std::{
  io::{BufRead, BufReader},
  sync::mpsc::Sender,
  thread::spawn,
};

use crate::{active_rule, BuildCommand, BuildEntry, BuildEvent, Debug, Origin};

use super::AppOptions;

pub struct Builder {
  options: AppOptions,
  tx_entries: Sender<Vec<BuildEntry>>,
  tx_events: Sender<BuildEvent>,
}

impl Builder {
  pub fn new(
    options: AppOptions,
    tx_entries: Sender<Vec<BuildEntry>>,
    tx_events: Sender<BuildEvent>,
  ) -> Self {
    Self {
      options,
      tx_entries,
      tx_events,
    }
  }
  /// The `cargo build` thread. It will run the [`BuildCommand`]
  /// and push output lines to [`BuildOutput`]
  pub fn run(self) {
    let args = self.options.build_args;
    crate::dbg!("build thread started: {:#?}", active_rule());
    match BuildCommand::spawn(args) {
      Ok(mut build) => {
        let _ = self.tx_events.send(BuildEvent::BuildStarted);
        Debug::log("spawned cargo process");
        let out_buf = BufReader::new(build.stdout.take().unwrap());
        let err_buf = BufReader::new(build.stderr.take().unwrap());

        let stderr_events = self.tx_entries.clone();
        let stdout_events = self.tx_entries.clone();
        let stdout_thread = spawn(move || {
          for line in out_buf.lines() {
            let line = line.expect("invalid output line");
            // Debug::log(format!("[stdout] {}", line));
            let _ = stdout_events.send(vec![BuildEntry::new(line, Origin::Stdout)]);
          }
        });
        let stderr_thread = spawn(move || {
          for line in err_buf.lines() {
            let line = line.expect("invalid error line");
            // Debug::log(format!("[stderr] {}", line));
            let t = vec![BuildEntry::new(line, Origin::Stderr)];
            let _ = stderr_events.send(t);
          }
        });
        // Debug::log("Waiting for stdout/err threads");
        stdout_thread
          .join()
          .expect("failed to join process reader thread");
        stderr_thread
          .join()
          .expect("failed to join process reader thread");
        // Debug::log("Done waiting for stdout/err threads");

        let exit_status = build.wait().expect("failed to wait for cargo");

        let _ = self.tx_events.send(BuildEvent::BuildFinished(exit_status));
        Debug::log(format!("Exit status: {}", exit_status));
      }
      Err(e) => Debug::log(format!("error: failed to spawn cargo build, {}", e)),
    }
    Debug::log("build thread stopped");
  }
}
