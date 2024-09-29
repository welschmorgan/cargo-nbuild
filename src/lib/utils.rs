use std::{
  sync::{Mutex, MutexGuard},
  time::{Duration, Instant},
};

use crate::{Error, ErrorKind};

pub trait TryLockFor<T> {
  fn try_lock_for(&self, dur: Duration) -> Result<MutexGuard<'_, T>, Error>;
}

impl<T> TryLockFor<T> for Mutex<T> {
  fn try_lock_for(&self, dur: Duration) -> Result<MutexGuard<'_, T>, Error> {
    let start = Instant::now();
    let end = start + dur;
    while Instant::now() < end {
      if let Ok(g) = self.try_lock() {
        return Ok(g);
      }
      std::thread::sleep(Duration::from_millis(10));
    }
    Err(Error::new(ErrorKind::LockTimeout, None, None))
  }
}
