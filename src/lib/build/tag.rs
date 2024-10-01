use std::{fmt::Display, ops::Range, path::Path};

use crate::CapturedMarker;

use super::Location;

/// Represent the kind of a BuildTag, put on each [`BuildEntry`]
#[derive(Debug, Clone, PartialEq, PartialOrd, Copy)]
pub enum BuildTagKind {
  /// A cargo warning
  Warning,
  /// A cargo error
  Error,
  /// A cargo note
  Note,
  /// Hide this entry from the UI
  Hidden,
  /// A marker's location
  Location,
}

impl Display for BuildTagKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self)
  }
}

/// Represent some extra information put on [`BuildEntry`]
#[derive(Debug, Clone)]
pub struct BuildTag {
  kind: BuildTagKind,
  marker: Option<CapturedMarker>,
  location: Option<Location>,
}

impl BuildTag {
  /// Construct a marker tag
  pub fn marker<C: AsRef<str>>(k: BuildTagKind, range: Range<usize>, capture: C) -> Self {
    Self {
      kind: k,
      marker: Some(CapturedMarker {
        range,
        text: capture.as_ref().to_string(),
      }),
      location: None,
    }
  }

  /// Construct a warning marker tag
  pub fn warning<C: AsRef<str>>(range: Range<usize>, capture: C) -> Self {
    Self::marker(BuildTagKind::Warning, range, capture)
  }

  /// Construct a error marker tag
  pub fn error<C: AsRef<str>>(range: Range<usize>, capture: C) -> Self {
    Self::marker(BuildTagKind::Error, range, capture)
  }

  /// Construct a error marker tag
  pub fn note<C: AsRef<str>>(range: Range<usize>, capture: C) -> Self {
    Self::marker(BuildTagKind::Note, range, capture)
  }

  /// Construct a hidden tag
  pub fn hidden() -> Self {
    Self {
      kind: BuildTagKind::Hidden,
      marker: None,
      location: None,
    }
  }

  /// Construct a location tag (next line after [`BuildTagKind::Error`]/[`BuildTagKind::Warning`] markers)
  pub fn location<P: AsRef<Path>>(path: P, line: Option<usize>, column: Option<usize>) -> Self {
    Self {
      kind: BuildTagKind::Location,
      marker: None,
      location: Some(Location::new(path.as_ref().to_path_buf(), line, column)),
    }
  }

  pub fn get_kind(&self) -> BuildTagKind {
    self.kind
  }

  pub fn get_marker(&self) -> Option<&CapturedMarker> {
    self.marker.as_ref()
  }

  pub fn get_location(&self) -> Option<&Location> {
    self.location.as_ref()
  }
}

impl PartialEq for BuildTag {
  fn eq(&self, other: &Self) -> bool {
    return self.kind == other.kind;
  }
}

impl PartialOrd for BuildTag {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    return self.kind.partial_cmp(&other.kind);
  }
}
