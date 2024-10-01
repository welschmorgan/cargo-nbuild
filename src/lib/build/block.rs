use std::ops::Range;

use crate::MarkerRef;

use super::BuildEntry;

#[derive(Clone, Debug, PartialEq)]
pub struct MarkedBlock<'a> {
  marker_id: usize,
  marker: MarkerRef<'a>,
  entry_range: Range<usize>,
  entries: Vec<&'a BuildEntry>,
}

impl<'a> MarkedBlock<'a> {
  pub fn new(
    marker_id: usize,
    marker: MarkerRef<'a>,
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

  pub fn marker(&self) -> &MarkerRef<'a> {
    &self.marker
  }
  pub fn marker_mut(&mut self) -> &mut MarkerRef<'a> {
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
