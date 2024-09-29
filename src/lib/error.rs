use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
pub enum ErrorKind {
    IO,
    LockPoisoned,
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

#[derive(Debug, Clone)]
pub struct Error {
    kind: ErrorKind,
    message: Option<String>,
    cause: Option<Box<Error>>,
}

impl Error {
    pub fn new(k: ErrorKind, msg: Option<String>, cause: Option<Box<Error>>) -> Self {
        Self {
            kind: k,
            message: msg,
            cause: cause,
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn message(&self) -> Option<&String> {
        self.message.as_ref()
    }

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

pub type Result<T> = std::result::Result<T, Error>;
