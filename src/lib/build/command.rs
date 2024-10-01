use std::{
  io,
  ops::{Deref, DerefMut},
  process::{Child, Command, Stdio},
};

/// Represent the `cargo build` process.
pub struct BuildCommand(Child);

impl BuildCommand {
  /// Spawn the process, setting piped stdout/stderr streams
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
