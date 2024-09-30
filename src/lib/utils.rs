use std::{
  io::{BufRead as _, BufReader, Read},
  sync::{Mutex, MutexGuard},
  time::{Duration, Instant},
};

use crate::{err, Error, ErrorKind};

/// A trait to support trying to lock a mutex for a certain amount of time
pub trait TryLockFor<T> {
  /// Try to lock the mutex for [`dur`] amount of time
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
    Err(err!(ErrorKind::LockTimeout))
  }
}

/// A batched line reader
pub struct BatchLineReader<R: ?Sized> {
  reader: Box<BufReader<R>>,
  has_more_batches: bool,
  max_time_per_batch: Option<Duration>,
  max_lines_per_batch: Option<usize>,
}

impl<R: Read> BatchLineReader<R> {
  pub fn new(r: R) -> Self {
    Self {
      reader: Box::new(BufReader::new(r)),
      has_more_batches: true,
      max_time_per_batch: None,
      max_lines_per_batch: None,
    }
  }

  pub fn has_more_batches(&self) -> bool {
    self.has_more_batches
  }

  pub fn next_line(&mut self) -> Option<String> {
    let mut buf = String::new();
    if let Ok(nbytes) = self.reader.read_line(&mut buf) {
      if nbytes == 0 {
        self.has_more_batches = false;
      }
      return Some(buf);
    }
    return None;
  }

  pub fn next_batch(&mut self) -> Option<Vec<String>> {
    let batch_start = Instant::now();
    let batch_end = self
      .max_time_per_batch
      .map(|max_time| batch_start + max_time);
    let mut ret = vec![];
    let mut limit_reached = false;
    loop {
      let time_limit_reached = match batch_end {
        Some(batch_end) => Instant::now() >= batch_end,
        None => false,
      };
      let line_limit_reached = match self.max_lines_per_batch {
        Some(line_limit) => ret.len() >= line_limit,
        None => false,
      };
      if time_limit_reached || line_limit_reached {
        limit_reached = true;
        break;
      }
      match self.next_line() {
        Some(line) => ret.push(line),
        None => break,
      }
    }
    if ret.len() == 0 && !limit_reached {
      // we didn't receive anything and no limit was reached
      // which means we encountered EOF
      return None;
    }
    return Some(ret);
  }
}
