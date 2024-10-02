use std::{
  ops::{Deref, DerefMut, Range},
  sync::Arc,
};

use lazy_static::lazy_static;
use ratatui::style::{Style, Stylize};
use regex::Regex;

use crate::{BuildEntry, BuildTag, BuildTagKind};

lazy_static! {
  /// Known build markers: errors, warnings and notes
  pub static ref BUILD_MARKERS: Arc<Vec<DeclaredMarker>> = Arc::new(vec![
    DeclaredMarker {
      tag: BuildTagKind::Error,
      regex: Regex::new(r"error(\[\w+\])?:").expect("invalid regular expression"),
      style: Style::default().red().bold(),
    },
    DeclaredMarker {
      tag: BuildTagKind::Note,
      regex: Regex::new(r"note(\[\w+\])?:").expect("invalid regular expression"),
      style: Style::default().blue().bold(),
    },
    DeclaredMarker {
      tag: BuildTagKind::Warning,
      regex: Regex::new(r"warning(\[\w+\])?:").expect("invalid regular expression"),
      style: Style::default().yellow().bold(),
    },
  ]);
}

pub fn known_marker<'a>(k: BuildTagKind) -> Option<&'a DeclaredMarker> {
  BUILD_MARKERS.iter().find(|m| m.tag == k)
}

pub fn must_know_marker<'a>(k: BuildTagKind) -> &'a DeclaredMarker {
  known_marker(k).expect("unknown marker")
}

/// The markers that were captured if the [`BuildEntry::message`] matched.
///
/// See [`Marker`] for the declaration counter-part
#[derive(Debug, Clone, PartialEq)]
pub struct CapturedMarker {
  /// The capture's range
  pub range: Range<usize>,
  /// The captured text
  pub text: String,
}

impl CapturedMarker {
  /// Construct a new instance
  pub fn new<C: AsRef<str>>(start: usize, capture: C) -> Self {
    Self {
      range: Range {
        start,
        end: start + capture.as_ref().len(),
      },
      text: capture.as_ref().to_string(),
    }
  }
}

/// Represent a marker definition
#[derive(Debug, Clone)]
pub struct DeclaredMarker {
  /// The tag kind
  pub tag: BuildTagKind,
  /// The regex used to capture text
  pub regex: Regex,
  /// The final style applied to the marker
  pub style: Style,
}

impl PartialEq for DeclaredMarker {
  fn eq(&self, other: &Self) -> bool {
    self.tag == other.tag
      && self.regex.as_str() == other.regex.as_str()
      && self.style == other.style
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkerRef<'a>(BuildTagKind, Option<&'a CapturedMarker>, &'a DeclaredMarker);

impl<'a> MarkerRef<'a> {
  pub fn new(
    kind: BuildTagKind,
    capture: Option<&'a CapturedMarker>,
    declared: &'a DeclaredMarker,
  ) -> Self {
    Self(kind, capture, declared)
  }

  pub fn known(kind: BuildTagKind, capture: Option<&'a CapturedMarker>) -> Self {
    Self::new(kind, capture, must_know_marker(kind))
  }

  pub fn kind(&self) -> BuildTagKind {
    self.0
  }

  pub fn captured(&self) -> Option<&'a CapturedMarker> {
    self.1
  }

  pub fn declared(&self) -> &'a DeclaredMarker {
    self.2
  }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MarkerSelection {
  pub marker_id: usize,
  pub entry_id: usize,
  pub region: Option<Range<usize>>,
}

impl MarkerSelection {
  pub fn new(marker_id: usize, entry_id: usize, text_selected: Option<Range<usize>>) -> Self {
    Self {
      marker_id,
      entry_id,
      region: text_selected,
    }
  }
}

/// Represent a list of markers extracted from [`BuildEntry`] tags
#[derive(Debug, Clone, PartialEq)]
pub struct Markers {
  /// The list of tags as a list of `(entry_id, marker_kind)` tuples
  tags: Vec<(usize, BuildTagKind)>,
  /// The currently selected marker, which corresponds to an item in the [`Markers::tags`] list
  selection: Option<MarkerSelection>,
}

impl Markers {
  /// Construct an empty markers list
  pub fn new() -> Self {
    Self {
      tags: Vec::new(),
      selection: None,
    }
  }

  /// Retrieve the list of tags
  pub fn tags(&self) -> &Vec<(usize, BuildTagKind)> {
    &self.tags
  }

  /// Retrieve the list of tags as a mutable reference
  pub fn tags_mut(&mut self) -> &mut Vec<(usize, BuildTagKind)> {
    &mut self.tags
  }

  /// Retrieve the currently selected marker.
  /// The value coresponds to an entry in the [`Markers::tags`] vector
  pub fn selection(&self) -> Option<&MarkerSelection> {
    self.selection.as_ref()
  }

  /// Retrieve the currently selected marker as a mutable ref.
  /// The value coresponds to an entry in the [`Markers::tags`] vector
  pub fn selection_mut(&mut self) -> &mut Option<MarkerSelection> {
    &mut self.selection
  }

  /// Retrieve the currently selected entry.
  pub fn selected_entry(&self) -> Option<usize> {
    if let Some(selected) = &self.selection {
      return self
        .tags
        .iter()
        .enumerate()
        .find_map(|(marker_id, (entry_id, _tag))| {
          if marker_id == selected.marker_id {
            return Some(entry_id);
          }
          None
        })
        .cloned();
    }
    None
  }

  /// Retrieve the [`BuildTagKind`] of the currently selected marker
  pub fn selected_kind(&self) -> Option<BuildTagKind> {
    if let Some(selected) = &self.selection {
      return self
        .tags
        .iter()
        .enumerate()
        .find_map(|(marker_id, (_entry_id, tag))| {
          if marker_id == selected.marker_id {
            return Some(tag);
          }
          None
        })
        .cloned();
    }
    None
  }

  /// Prepare markers of each [`BuildEntry`].
  ///
  /// Markers are messages that cargo emits like `^(warning|error|note):`
  pub fn prepare(entry: &mut BuildEntry) {
    for known_marker in BUILD_MARKERS.iter() {
      if let Some(m) = known_marker.regex.find(&entry.message()) {
        entry.set_tag(BuildTag::marker(
          known_marker.tag,
          m.range(),
          m.as_str().to_string(),
        ));
      }
    }
  }

  /// Retrieve a marker's `(entry_id, marker_kind)` value by it's id
  pub fn entry_for_marker(&self, id: usize) -> Option<&(usize, BuildTagKind)> {
    self.tags.get(id)
  }

  /// Retrieve a marker's `(entry_id, marker_kind)` value by it's id as a mutable reference
  pub fn entry_for_marker_mut(&mut self, id: usize) -> Option<&mut (usize, BuildTagKind)> {
    self.tags.get_mut(id)
  }

  /// Select a specific marker
  pub fn select(&mut self, mut id: usize, text: Option<Range<usize>>) {
    if self.tags.is_empty() {
      self.selection = None;
    } else {
      id = id.min(self.tags.len().saturating_sub(1));
      self.selection = Some(MarkerSelection::new(id, self.tags[id].0, text));
    }
  }

  /// Unselect marker
  pub fn unselect(&mut self) {
    self.selection = None;
  }

  /// Retrieve the marker before the one currently selected.
  ///
  /// If no markers were previously selected, it selects the first marker.
  /// If the currently selected marker is the first one, it selects the first marker again.
  /// Otherwise it just decrements the currently selected marker.
  pub fn previous_selection(&self) -> Option<MarkerSelection> {
    let prev_marker = match self.tags.is_empty() {
      true => return None,
      false => match self.selection.as_ref() {
        Some(cur) => cur.marker_id.saturating_sub(1),
        None => 0,
      },
    };
    let entry_id = self
      .tags
      .get(prev_marker)
      .map(|tag| tag.0)
      .unwrap_or_default();
    Some(MarkerSelection {
      marker_id: prev_marker,
      entry_id,
      ..Default::default()
    })
  }

  /// Retrieve the marker after the one currently selected.
  ///
  /// If no markers were previously selected, it selects the first marker.
  /// If the currently selected marker is the last one, it selects the last marker again.
  /// Otherwise it just increments the currently selected marker.
  pub fn next_selection(&self) -> Option<MarkerSelection> {
    let prev_marker = match self.tags.is_empty() {
      true => return None,
      false => match self.selection.as_ref() {
        Some(cur) => {
          if cur.marker_id < self.tags.len().saturating_sub(1) {
            cur.marker_id.saturating_add(1)
          } else {
            self.tags.len().saturating_sub(1)
          }
        }
        None => 0,
      },
    };
    let entry_id = self
      .tags
      .get(prev_marker)
      .map(|tag| tag.0)
      .unwrap_or_default();
    Some(MarkerSelection {
      marker_id: prev_marker,
      entry_id,
      ..Default::default()
    })
  }

  /// Retrieve the marker before the one currently selected.
  ///
  /// If no markers were previously selected, it selects the first marker.
  /// If the currently selected marker is the first one, it selects the first marker again.
  /// Otherwise it just decrements the currently selected marker.
  pub fn select_previous(&mut self) -> Option<&MarkerSelection> {
    self.selection = self.previous_selection();
    self.selection.as_ref()
  }

  /// Retrieve the marker after the one currently selected.
  ///
  /// If no markers were previously selected, it selects the first marker.
  /// If the currently selected marker is the last one, it selects the last marker again.
  /// Otherwise it just increments the currently selected marker.
  pub fn select_next(&mut self) -> Option<&MarkerSelection> {
    self.selection = self.next_selection();
    self.selection.as_ref()
  }

  /// Select the first marker
  pub fn select_first(&mut self) -> Option<&MarkerSelection> {
    self.select(0, None);
    self.selection.as_ref()
  }

  /// Select the last marker
  pub fn select_last(&mut self) -> Option<&MarkerSelection> {
    self.select(self.tags.len(), None);
    self.selection.as_ref()
  }
}

impl AsRef<Vec<(usize, BuildTagKind)>> for Markers {
  fn as_ref(&self) -> &Vec<(usize, BuildTagKind)> {
    &self.tags
  }
}

impl Deref for Markers {
  type Target = Vec<(usize, BuildTagKind)>;

  fn deref(&self) -> &Self::Target {
    &self.tags
  }
}

impl DerefMut for Markers {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.tags
  }
}

impl From<&[BuildEntry]> for Markers {
  fn from(entries: &[BuildEntry]) -> Self {
    Self {
      tags: entries
        .iter()
        .enumerate()
        // .inspect(|(id, entry)| crate::dbg!(format!("entry #{}: {:?}", id, entry.tags())))
        .filter_map(|(id, entry)| entry.marker().map(|marker| (id, marker.kind())))
        .collect::<Vec<_>>(),
      selection: None,
    }
  }
}

impl From<&Vec<BuildEntry>> for Markers {
  fn from(entries: &Vec<BuildEntry>) -> Self {
    Self::from(entries.as_slice())
  }
}

impl Default for Markers {
  fn default() -> Self {
    Self {
      tags: Default::default(),
      selection: Default::default(),
    }
  }
}

#[cfg(test)]
mod tests {
  use std::ops::Range;

  use crate::{
    must_know_marker, BuildEntry, BuildTag, BuildTagKind, CapturedMarker, MarkerRef,
    MarkerSelection, Origin,
  };

  use super::Markers;

  #[test]
  fn prepare_error() {
    let mut entry = BuildEntry::new("error: test", Origin::default());
    Markers::prepare(&mut entry);
    assert_eq!(
      entry.marker(),
      Some(MarkerRef::new(
        BuildTagKind::Error,
        Some(&CapturedMarker::new(0, "error:")),
        must_know_marker(BuildTagKind::Error)
      ))
    )
  }

  #[test]
  fn prepare_warning() {
    let mut entry = BuildEntry::new("warning: test", Origin::default());
    Markers::prepare(&mut entry);
    assert_eq!(
      entry.marker(),
      Some(MarkerRef::new(
        BuildTagKind::Warning,
        Some(&CapturedMarker::new(0, "warning:")),
        must_know_marker(BuildTagKind::Warning)
      ))
    )
  }

  #[test]
  fn prepare_note() {
    let mut entry = BuildEntry::new("note: test", Origin::default());
    Markers::prepare(&mut entry);
    let marker = entry.marker();
    assert_eq!(
      marker,
      Some(MarkerRef::new(
        BuildTagKind::Note,
        Some(&CapturedMarker::new(0, "note:")),
        must_know_marker(BuildTagKind::Note)
      ))
    )
  }

  #[test]
  fn from_entries() {
    let entries = vec![
      BuildEntry::new("error: test error", Origin::default())
        .with_tags([BuildTag::error(Range { start: 0, end: 6 }, "error:")]),
      BuildEntry::new("and a non-marker", Origin::default()),
    ];
    let markers = Markers::from(&entries);
    assert_eq!(
      markers,
      Markers {
        tags: vec![(0, BuildTagKind::Error)],
        selection: None
      }
    )
  }

  #[test]
  fn selected() {
    let entries = vec![
      BuildEntry::new("error: test error", Origin::default())
        .with_tags([BuildTag::error(Range { start: 0, end: 6 }, "error:")]),
      BuildEntry::new("and a non-marker", Origin::default()),
      BuildEntry::new("warning: test warning", Origin::default())
        .with_tags([BuildTag::warning(Range { start: 0, end: 7 }, "warning:")]),
    ];
    let mut markers = Markers::from(&entries);
    markers.select(1, None);
    assert_eq!(markers.selection, Some(MarkerSelection::new(1, 2, None)));
    assert_eq!(markers.selected_entry(), Some(2));
    assert_eq!(markers.selected_kind(), Some(BuildTagKind::Warning));
  }

  #[test]
  fn entry_for_marker() {
    let entries = vec![
      BuildEntry::new("error: test error", Origin::default())
        .with_tags([BuildTag::error(Range { start: 0, end: 6 }, "error:")]),
      BuildEntry::new("and a non-marker", Origin::default()),
      BuildEntry::new("warning: test warning", Origin::default())
        .with_tags([BuildTag::warning(Range { start: 0, end: 7 }, "warning:")]),
    ];
    let markers = Markers::from(&entries);
    let entry = markers.entry_for_marker(1);
    assert_eq!(entry, Some(&(2, BuildTagKind::Warning)))
  }

  #[test]
  fn select_next() {
    let entries = vec![
      BuildEntry::new("error: test error", Origin::default())
        .with_tags([BuildTag::error(Range { start: 0, end: 6 }, "error:")]),
      BuildEntry::new("and a non-marker", Origin::default()),
      BuildEntry::new("warning: test warning", Origin::default())
        .with_tags([BuildTag::warning(Range { start: 0, end: 7 }, "warning:")]),
    ];
    let mut markers = Markers::from(&entries);
    // first time goes from None -> Some(0)
    assert_eq!(
      markers.select_next(),
      Some(&MarkerSelection::new(0, 0, None))
    );
    // second time goes from Some(0) -> Some(1)
    assert_eq!(
      markers.select_next(),
      Some(&MarkerSelection::new(1, 2, None))
    );
    // third time goes from Some(1) -> Some(1) as it is out-of-bounds
    assert_eq!(
      markers.select_next(),
      Some(&MarkerSelection::new(1, 2, None))
    );
  }

  #[test]
  fn select_previous() {
    let entries = vec![
      BuildEntry::new("error: test error", Origin::default())
        .with_tags([BuildTag::error(Range { start: 0, end: 6 }, "error:")]),
      BuildEntry::new("and a non-marker", Origin::default()),
      BuildEntry::new("warning: test warning", Origin::default())
        .with_tags([BuildTag::warning(Range { start: 0, end: 7 }, "warning:")]),
    ];
    let mut markers = Markers::from(&entries);
    // first time goes from None -> Some(0)
    assert_eq!(
      markers.select_previous(),
      Some(&MarkerSelection::new(0, 0, None))
    );
    // second time goes from Some(0) -> Some(0)
    assert_eq!(
      markers.select_previous(),
      Some(&MarkerSelection::new(0, 0, None))
    );
  }
}
