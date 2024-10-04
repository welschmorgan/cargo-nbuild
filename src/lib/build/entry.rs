use std::time::Instant;

use crate::MarkerRef;

use super::{rules, BuildTag, BuildTagKind, Origin, Rule, DEFAULT_RULES};

/// Represent an output line written by the cargo build process [`BuildCommand`]
#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct BuildEntry {
  created_at: Instant,
  message: String,
  origin: Origin,
  tags: Vec<BuildTag>,
}

impl BuildEntry {
  /// Construct a new entry
  pub fn new<M: AsRef<str>>(msg: M, orig: Origin) -> Self {
    Self {
      created_at: Instant::now(),
      message: msg.as_ref().to_string(),
      origin: orig,
      tags: vec![],
    }
  }

  pub fn with_tags<I: IntoIterator<Item = BuildTag>>(mut self, tags: I) -> Self {
    self.tags.extend(tags);
    self
  }

  pub fn with_created_at(mut self, at: Instant) -> Self {
    self.created_at = at;
    self
  }

  /// Retrieve the [`Instant`] this entry was created
  pub fn created_at(&self) -> &Instant {
    &self.created_at
  }

  /// Retrieve the line's content
  pub fn message(&self) -> &String {
    &self.message
  }

  /// Retrieve the [`Origin`] this entry was created from
  pub fn origin(&self) -> Origin {
    self.origin
  }

  /// Checks if this entry has a [`BuildTagKind::Error`] attached to it.
  pub fn is_error(&self) -> bool {
    self.has_tag(BuildTagKind::Error)
  }

  /// Checks if this entry has a [`BuildTagKind::Warning`] attached to it.
  pub fn is_warning(&self) -> bool {
    self.has_tag(BuildTagKind::Warning)
  }

  /// Checks if this entry has a [`BuildTagKind::Warning`] attached to it.
  pub fn is_note(&self) -> bool {
    self.has_tag(BuildTagKind::Note)
  }

  /// Define a [`BuildTag`]
  pub fn set_tag(&mut self, t: BuildTag) {
    if let Some(tag) = self.tag_mut(t.get_kind()) {
      *tag = t;
    } else {
      self.tags.push(t);
    }
  }

  /// Retrieve a mutable ref to an existing [`BuildTag`]
  pub fn tag_mut(&mut self, t: BuildTagKind) -> Option<&mut BuildTag> {
    self.tags.iter_mut().find(|cur| cur.get_kind() == t)
  }

  /// Retrieve a ref to an existing [`BuildTag`]
  pub fn tag(&self, t: BuildTagKind) -> Option<&BuildTag> {
    self.tags.iter().find(|cur| cur.get_kind() == t)
  }

  /// Retrieve any [`Marker`] associated to this tag
  pub fn first_marker<'a>(&'a self) -> Option<&MarkerRef> {
    if let Some(tag) = self.tags.iter().find(|tag| tag.get_marker().is_some()) {
      return tag.get_marker();
    }
    None
  }

  /// Retrieve all tags
  pub fn tags(&self) -> &Vec<BuildTag> {
    &self.tags
  }

  /// Retrieve all mutable tags
  pub fn tags_mut(&mut self) -> &mut Vec<BuildTag> {
    &mut self.tags
  }

  /// Retrieve the [`BuildTagKind::Location`] tag assigned to this node ([`Location`])
  pub fn location(&self) -> Option<&BuildTag> {
    self.tag(BuildTagKind::Location)
  }

  /// Check if this entry contains a tag by it's [`BuildTagKind`]
  pub fn has_tag(&self, k: BuildTagKind) -> bool {
    for t in &self.tags {
      if t.get_kind() == k {
        return true;
      }
    }
    return false;
  }

  /// Format a [`Location`] string if it is defined on this [`BuildEntry`]
  pub fn location_str(&self) -> Option<String> {
    self.location().map(|t| {
      format!(
        "{}",
        match t.get_location() {
          Some(loc) => format!(
            "{}{}{}",
            loc.path().display(),
            match loc.line() {
              Some(line) => format!(":{}", line),
              None => String::new(),
            },
            match loc.column() {
              Some(column) => format!(":{}", column),
              None => String::new(),
            }
          ),
          _ => String::new(),
        }
      )
    })
  }
}

impl<S: AsRef<str>> From<S> for BuildEntry {
  fn from(value: S) -> Self {
    BuildEntry::new(value.as_ref(), Origin::default())
  }
}
