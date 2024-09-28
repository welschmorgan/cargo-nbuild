use std::{
  io,
  ops::{Deref, DerefMut},
  process::{Child, Command, Stdio},
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
