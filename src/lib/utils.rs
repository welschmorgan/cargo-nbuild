use std::{
  fs::File,
  sync::{Mutex, MutexGuard},
  time::{Duration, Instant},
};

pub trait TryLockFor<T> {
  fn try_lock_for(&self, dur: Duration) -> Result<MutexGuard<'_, T>, ()>;
}

impl<T> TryLockFor<T> for Mutex<T> {
  fn try_lock_for(&self, dur: Duration) -> Result<MutexGuard<'_, T>, ()> {
    let start = Instant::now();
    let end = start + dur;
    while Instant::now() < end {
      if let Ok(g) = self.try_lock() {
        return Ok(g);
      }
      std::thread::sleep(Duration::from_millis(10));
    }
    Err(())
  }
}
