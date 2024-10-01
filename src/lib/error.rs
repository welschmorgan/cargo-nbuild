use std::fmt::Display;

use crate::Location;

/// The known error messages kind
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
pub enum ErrorKind {
  /// An IO error: something couln't be read or written
  IO,
  /// The lock is poisoned
  LockPoisoned,
  /// A timeout was reached while acquiring the lock
  LockTimeout,
  /// Parsing failed
  Parsing,
}

impl Display for ErrorKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{}",
      match self {
        Self::IO => "i/o",
        Self::LockPoisoned => "lock poisoned",
        Self::LockTimeout => "lock timeout",
        Self::Parsing => "parsing failed",
      }
    )
  }
}

/// Represent an error
#[derive(Debug, Clone)]
pub struct Error {
  kind: ErrorKind,
  message: Option<String>,
  cause: Option<Box<Error>>,
  location: Option<Location>,
}

impl Error {
  /// Construct a new Error
  pub fn new(
    k: ErrorKind,
    msg: Option<String>,
    cause: Option<Box<Error>>,
    location: Option<Location>,
  ) -> Self {
    Self {
      kind: k,
      message: msg,
      cause,
      location,
    }
  }

  pub fn with_message<M: AsRef<str>>(mut self, m: M) -> Self {
    self.message = m.as_ref().to_string().into();
    self
  }

  pub fn with_cause(mut self, cause: Error) -> Self {
    self.cause = Some(Box::new(cause));
    self
  }

  pub fn with_location(mut self, location: Location) -> Self {
    self.location = Some(location);
    self
  }

  /// Retrieve the error kind
  pub fn kind(&self) -> ErrorKind {
    self.kind
  }

  /// Retrieve the error message
  pub fn message(&self) -> Option<&String> {
    self.message.as_ref()
  }

  /// Retrieve the error cause if any
  pub fn cause(&self) -> Option<&Box<Error>> {
    self.cause.as_ref()
  }

  /// Retrieve the error cause if any
  pub fn location(&self) -> Option<&Location> {
    self.location.as_ref()
  }
}

impl std::error::Error for Error {}
impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{}{}{}",
      self.kind(),
      match self.message() {
        Some(msg) => format!(": {}", msg),
        None => String::new(),
      },
      match self.cause() {
        Some(cause) => format!("\nCaused by: {}", cause),
        None => String::new(),
      }
    )
  }
}

/// Represent an internal result
pub type Result<T> = std::result::Result<T, Error>;

#[macro_export]
macro_rules! err {
  ($kind:expr) => {
    $crate::Error::new($kind, None, None, Some($crate::here!()))
  };

  (with_cause $cause:expr, $kind:expr, $msg:expr) => {
    $crate::Error::new(
      $kind,
      Some(format!("{}", $msg)),
      Some(Box::new($cause)),
      Some($crate::here!()),
    )
  };

  ($kind:expr, $fmt:expr$(, $args:expr)*) => {
    $crate::Error::new(
      $kind,
      Some(format!("{}", format_args!($fmt, $( $args ),*))),
      None,
      Some($crate::here!()),
    )
  };
}
