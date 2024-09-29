use std::fmt::Display;

/// The known error messages kind
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
pub enum ErrorKind {
  /// An IO error: something couln't be read or written
  IO,
  /// The lock is poisoned
  LockPoisoned,
  /// A timeout was reached while acquiring the lock
  LockTimeout,
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
}

impl Error {
  /// Construct a new Error
  pub fn new(k: ErrorKind, msg: Option<String>, cause: Option<Box<Error>>) -> Self {
    Self {
      kind: k,
      message: msg,
      cause: cause,
    }
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
