use std::ops::Range;

use crate::MarkerRef;

use super::BuildEntry;

#[derive(Clone, Debug, PartialEq)]
pub struct MarkedBlock<'a> {
  marker: MarkerRef<'a>,
  range: Range<usize>,
  entries: Vec<&'a BuildEntry>,
}

impl<'a> MarkedBlock<'a> {
  pub fn new(marker: MarkerRef<'a>, range: Range<usize>, entries: Vec<&'a BuildEntry>) -> Self {
    Self {
      marker,
      range,
      entries,
    }
  }

  pub fn marker(&self) -> &MarkerRef<'a> {
    &self.marker
  }
  pub fn marker_mut(&mut self) -> &mut MarkerRef<'a> {
    &mut self.marker
  }

  pub fn range(&self) -> Range<usize> {
    self.range.clone()
  }
  pub fn range_mut(&mut self) -> &mut Range<usize> {
    &mut self.range
  }

  pub fn entries(&self) -> &Vec<&'a BuildEntry> {
    &self.entries
  }
  pub fn entries_mut(&mut self) -> &mut Vec<&'a BuildEntry> {
    &mut self.entries
  }
}
