use std::{
  collections::VecDeque,
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
  static ref debug_log_buf: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
}

pub struct Debug {}

impl Debug {
  pub fn log<S: AsRef<str>>(msg: S) {
    if let Ok(mut f) = debug_log_handle.try_lock_for(Duration::from_millis(50)) {
      if let Ok(mut buf) = debug_log_buf.try_lock_for(Duration::from_millis(50)) {
        while let Some(e) = buf.pop_front() {
          let _ = write!(f, "{}\n", e.as_str().trim());
          let _ = f.flush();
        }
      }
      let _ = write!(f, "{}\n", msg.as_ref().trim());
      let _ = f.flush();
    } else {
      if let Ok(mut buf) = debug_log_buf.try_lock_for(Duration::from_millis(50)) {
        buf.push_back(msg.as_ref().to_string())
      }
    }
  }
}

#[macro_export]
macro_rules! dbg {
  ($msg:expr) => {
    $crate::Debug::log(format!("{}", $msg))
  };

  ($($args:expr),+) => {
    $crate::Debug::log(format!("{}", format_args!($( $args, )+)))
  };
}
