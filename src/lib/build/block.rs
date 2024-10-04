use std::{fmt::Display, ops::Range};

use crate::MarkerRef;

use super::BuildEntry;

#[derive(Clone, Debug, PartialEq)]
pub struct MarkedBlock<'a> {
  marker_id: usize,
  marker: MarkerRef,
  entry_range: Range<usize>,
  entries: Vec<&'a BuildEntry>,
}

impl<'a> Display for MarkedBlock<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "Block #{}:{}",
      self.marker_id,
      match self.entries.is_empty() {
        true => String::new(),
        false => format!(
          " {:?}:{}",
          self.range(),
          self
            .entries
            .iter()
            .map(|entry| format!("\n  | {}", entry.message()))
            .collect::<Vec<_>>()
            .join("")
        ),
      }
    )
  }
}
impl<'a> MarkedBlock<'a> {
  pub fn new(
    marker_id: usize,
    marker: MarkerRef,
    entry_range: Range<usize>,
    entries: Vec<&'a BuildEntry>,
  ) -> Self {
    Self {
      marker_id,
      marker,
      entry_range,
      entries,
    }
  }

  pub fn lines(&self) -> Vec<&str> {
    self
      .entries
      .iter()
      .map(|e| e.message().as_str())
      .collect::<Vec<_>>()
  }

  pub fn content(&self) -> String {
    self.lines().join("\n")
  }

  pub fn marker(&self) -> &MarkerRef {
    &self.marker
  }
  pub fn marker_mut(&mut self) -> &mut MarkerRef {
    &mut self.marker
  }

  pub fn range(&self) -> Range<usize> {
    self.entry_range.clone()
  }
  pub fn range_mut(&mut self) -> &mut Range<usize> {
    &mut self.entry_range
  }

  pub fn marker_id(&self) -> usize {
    self.marker_id
  }
  pub fn marker_id_mut(&mut self) -> &mut usize {
    &mut self.marker_id
  }

  pub fn entries(&self) -> &Vec<&'a BuildEntry> {
    &self.entries
  }
  pub fn entries_mut(&mut self) -> &mut Vec<&'a BuildEntry> {
    &mut self.entries
  }
}
