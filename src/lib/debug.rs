use std::{
  fs::File,
  io::Write,
  sync::{Arc, Mutex},
  time::Duration,
};

use lazy_static::lazy_static;

use crate::TryLockFor;

pub const DEBUG_FILE_PATH: &'static str = ".cargo-nbuild.log";

lazy_static! {
  static ref debug_log_handle: Arc<Mutex<File>> = Arc::new(Mutex::new(
    File::create(DEBUG_FILE_PATH).expect("failed to create debug log")
  ));
}

pub struct Debug;

impl Debug {
  pub fn log<S: AsRef<str>>(msg: S) {
    let mut f = debug_log_handle
      .try_lock_for(Duration::from_millis(50))
      .unwrap();
    let _ = write!(f, "{}\n", msg.as_ref().trim());
    let _ = f.flush();
  }
}
