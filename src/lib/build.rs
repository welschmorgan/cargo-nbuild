use std::{
  io,
  ops::{Deref, DerefMut},
  process::{Child, Command, Stdio},
  sync::mpsc::{channel, Receiver, Sender},
  time::Instant,
};

use ratatui::{
  style::Stylize,
  text::{Line, Span},
};

pub struct BuildCommand(Child);

impl BuildCommand {
  pub fn spawn(args: Vec<String>) -> io::Result<Self> {
    let child = Command::new("cargo")
      .arg("build")
      .args(args)
      .stderr(Stdio::piped())
      .stdout(Stdio::piped())
      .spawn()?;

    Ok(BuildCommand(child))
  }
}

impl Deref for BuildCommand {
  type Target = Child;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for BuildCommand {
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

#[derive(Default)]
pub struct BuildOutput {
  entries: Vec<BuildEntry>,
  // lines: Vec<Line<'a>>,
}

impl BuildOutput {
  pub fn push(mut self, e: BuildEntry) -> Self {
    self.entries.push(e);
    self
  }

  pub fn pull(mut self, from: &Receiver<BuildEntry>) -> Self {
    while let Ok(entry) = from.try_recv() {
      self = self.push(entry);
    }
    self
  }

  pub fn prepare(&self) -> Vec<Line<'_>> {
    let new_lines = self
      .entries
      .iter()
      .map(|e| {
        let mut line = Line::default();
        if e.message().starts_with("warning:") {
          line = line.spans(["warning:".bold().yellow(), e.message()[8..].into()]);
        } else if e.message().starts_with("error:") {
          line = line.spans(["error:".bold().yellow(), e.message()[7..].into()]);
        } else {
          line.push_span(Span::from(e.message().as_str()));
        }
        line
      })
      .collect::<Vec<_>>();
    new_lines
  }

  pub fn entries(&self) -> &Vec<BuildEntry> {
    &self.entries
  }
}
