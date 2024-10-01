use std::{
  fmt::Display,
  path::{Path, PathBuf},
  str::FromStr,
};

use crate::{Error, ErrorKind};

/// Represent a source-code location. Captured from Cargo's output
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Location {
  /// The file
  path: PathBuf,
  /// The line
  line: Option<usize>,
  /// The column
  column: Option<usize>,
}

impl Location {
  pub fn new<P: AsRef<Path>>(path: P, line: Option<usize>, column: Option<usize>) -> Self {
    Self {
      path: path.as_ref().to_path_buf(),
      line,
      column,
    }
  }

  pub fn path(&self) -> &PathBuf {
    &self.path
  }
  pub fn path_mut(&mut self) -> &mut PathBuf {
    &mut self.path
  }

  pub fn line(&self) -> Option<usize> {
    self.line
  }
  pub fn line_mut(&mut self) -> &mut Option<usize> {
    &mut self.line
  }

  pub fn column(&self) -> Option<usize> {
    self.column
  }
  pub fn column_mut(&mut self) -> &mut Option<usize> {
    &mut self.column
  }
}

impl FromStr for Location {
  type Err = crate::Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let parts = s.split(':').collect::<Vec<_>>();
    let path: PathBuf = parts[0].into();
    let mut line = None;
    let mut column = None;
    if parts.len() > 1 {
      line = match parts[1].parse::<usize>() {
        Ok(line_num) => Some(line_num),
        Err(e) => {
          crate::dbg!("error: failed to parse line num from '{}', {}", parts[1], e);
          None
        }
      };
    }
    if parts.len() > 2 {
      column = match parts[2].parse::<usize>() {
        Ok(line_num) => Some(line_num),
        Err(e) => {
          return Err(Error::new(
            ErrorKind::Parsing,
            Some(format!(
              "error: failed to parse column num from '{}', {}",
              parts[2], e
            )),
            None,
            Some(crate::here!()),
          ));
        }
      };
    }
    return Ok(Location { path, line, column });
  }
}

impl Display for Location {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{}{}{}",
      self.path.display(),
      match self.line {
        Some(l) => format!(": {}", l),
        None => String::new(),
      },
      match self.line {
        Some(_) => match self.column {
          Some(c) => format!(": {}", c),
          None => String::new(),
        },
        None => String::new(),
      }
    )
  }
}

#[macro_export]
macro_rules! here {
  () => {
    $crate::Location::new(
      std::path::PathBuf::from(file!()),
      Some(line!() as usize),
      Some(column!() as usize),
    )
  };
}
