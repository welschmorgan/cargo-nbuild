use std::io::{stderr, stdin, stdout, Read, Write};

/// Represent a stream origin: either [`std::io::Stdout`] or [`std::io::Stderr`]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum Origin {
  Stdin,
  Stdout,
  Stderr,
}

impl Default for Origin {
  fn default() -> Self {
    Self::Stdout
  }
}

impl Origin {
  pub fn reader(&self) -> Box<dyn Read> {
    match self {
      Self::Stdin => Box::new(stdin()),
      _ => panic!("stream {:?} is not readable", self),
    }
  }

  pub fn writer(&self) -> Box<dyn Write> {
    match self {
      Self::Stdout => Box::new(stdout()),
      Self::Stderr => Box::new(stderr()),
      _ => panic!("stream {:?} is not writable", self),
    }
  }
}
