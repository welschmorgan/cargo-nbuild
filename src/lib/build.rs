use std::{
  io,
  ops::{Deref, DerefMut},
  process::{Child, Command, Stdio},
  sync::mpsc::{channel, Receiver, Sender},
  time::Instant,
};

pub struct CargoBuild(Child);

impl CargoBuild {
  pub fn spawn(args: Vec<String>) -> io::Result<Self> {
    let child = Command::new("cargo")
      .arg("build")
      .args(args)
      .stderr(Stdio::piped())
      .stdout(Stdio::piped())
      .spawn()?;

    Ok(CargoBuild(child))
  }
}

impl Deref for CargoBuild {
  type Target = Child;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for CargoBuild {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum Origin {
  Stdout,
  Stderr,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct BuildEntry {
  created_at: Instant,
  message: String,
  origin: Origin,
}

impl BuildEntry {
  pub fn new<M: AsRef<str>>(msg: M, orig: Origin) -> Self {
    Self {
      created_at: Instant::now(),
      message: msg.as_ref().to_string(),
      origin: orig,
    }
  }

  pub fn created_at(&self) -> &Instant {
    &self.created_at
  }

  pub fn message(&self) -> &String {
    &self.message
  }

  pub fn origin(&self) -> Origin {
    self.origin
  }
}
