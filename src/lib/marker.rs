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
  pub static ref BUILD_MARKERS: Arc<Vec<Marker>> = Arc::new(vec![
    Marker {
      tag: BuildTagKind::Error,
      regex: Regex::new(r"error(\[\w+\])?:").expect("invalid regular expression"),
      style: Style::default().red().bold(),
    },
    Marker {
      tag: BuildTagKind::Note,
      regex: Regex::new(r"note(\[\w+\])?:").expect("invalid regular expression"),
      style: Style::default().blue().bold(),
    },
    Marker {
      tag: BuildTagKind::Warning,
      regex: Regex::new(r"warning(\[\w+\])?:").expect("invalid regular expression"),
      style: Style::default().yellow().bold(),
    },
  ]);
}

/// The markers that were captured if the [`BuildEntry::message`] matched.
///
/// See [`Marker`] for the declaration counter-part
#[derive(Debug, Clone, PartialEq)]
pub struct CapturedMarker {
  /// The capture's range
  pub range: Range<usize>,
  /// The captured text
  pub capture: String,
}

impl CapturedMarker {
  /// Construct a new instance
  pub fn new<C: AsRef<str>>(start: usize, capture: C) -> Self {
    Self {
      range: Range {
        start,
        end: start + capture.as_ref().len(),
      },
      capture: capture.as_ref().to_string(),
    }
  }
}

/// Represent a marker definition
#[derive(Debug, Clone)]
pub struct Marker {
  /// The tag kind
  pub tag: BuildTagKind,
  /// The regex used to capture text
  pub regex: Regex,
  /// The final style applied to the marker
  pub style: Style,
}

/// Represent a list of markers extracted from [`BuildEntry`] tags
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Markers {
  /// The list of tags as a list of `(entry_id, marker_kind)` tuples
  tags: Vec<(usize, BuildTagKind)>,
  /// The currently selected marker, which corresponds to an item in the [`Markers::tags`] list
  selected: Option<usize>,
}

impl Markers {
  /// Construct an empty markers list
  pub fn new() -> Self {
    Self {
      tags: Vec::new(),
      selected: None,
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
  pub fn selected(&self) -> Option<usize> {
    self.selected
  }

  /// Retrieve the currently selected marker as a mutable ref.
  /// The value coresponds to an entry in the [`Markers::tags`] vector
  pub fn selected_mut(&mut self) -> &mut Option<usize> {
    &mut self.selected
  }

  /// Retrieve the currently selected entry.
  pub fn selected_entry(&self) -> Option<usize> {
    if let Some(selected) = self.selected {
      return self
        .tags
        .iter()
        .enumerate()
        .find_map(|(marker_id, (entry_id, _tag))| {
          if marker_id == selected {
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
    if let Some(selected) = self.selected {
      return self
        .tags
        .iter()
        .enumerate()
        .find_map(|(marker_id, (_entry_id, tag))| {
          if marker_id == selected {
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
  pub fn select(&mut self, mut id: usize) {
    if self.tags.is_empty() {
      self.selected = None;
    } else {
      id = id.min(self.tags.len().saturating_sub(1));
      self.selected = Some(id);
    }
  }

  /// Unselect marker
  pub fn unselect(&mut self) {
    self.selected = None;
  }

  /// Retrieve the marker before the one currently selected.
  ///
  /// If no markers were previously selected, it selects the first marker.
  /// If the currently selected marker is the first one, it selects the first marker again.
  /// Otherwise it just decrements the currently selected marker.
  pub fn previous_selected(&self) -> Option<usize> {
    match self.selected {
      Some(cur) => Some(cur.saturating_sub(1)),
      None => Some(0),
    }
  }

  /// Retrieve the marker after the one currently selected.
  ///
  /// If no markers were previously selected, it selects the first marker.
  /// If the currently selected marker is the last one, it selects the last marker again.
  /// Otherwise it just increments the currently selected marker.
  pub fn next_selected(&self) -> Option<usize> {
    match self.selected {
      Some(cur) => {
        if self.tags.len() > 1 && cur < self.tags.len() - 1 {
          Some(cur.saturating_add(1))
        } else {
          Some(cur)
        }
      }
      None => Some(0),
    }
  }

  /// Retrieve the marker after the one currently selected.
  ///
  /// If no markers were previously selected, it selects the first marker.
  /// If the currently selected marker is the last one, it selects the last marker again.
  /// Otherwise it just increments the currently selected marker.
  pub fn select_next(&mut self) -> usize {
    self.selected = self.next_selected();
    self.selected.unwrap()
  }

  /// Select the last marker
  pub fn select_last(&mut self) -> Option<usize> {
    self.select(self.tags.len());
    self.selected
  }

  /// Select the first marker
  pub fn select_first(&mut self) -> Option<usize> {
    self.select(0);
    self.selected
  }

  /// Retrieve the marker before the one currently selected.
  ///
  /// If no markers were previously selected, it selects the first marker.
  /// If the currently selected marker is the first one, it selects the first marker again.
  /// Otherwise it just decrements the currently selected marker.
  pub fn select_previous(&mut self) -> usize {
    self.selected = self.previous_selected();
    self.selected.unwrap()
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
        .inspect(|(id, entry)| crate::dbg!(format!("entry #{}: {:?}", id, entry.tags())))
        .filter_map(|(id, entry)| entry.marker().map(|(tag, _marker)| (id, tag)))
        .collect::<Vec<_>>(),
      selected: None,
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
      selected: Default::default(),
    }
  }
}

#[cfg(test)]
mod tests {
  use std::ops::Range;

  use crate::{BuildEntry, BuildTag, BuildTagKind, CapturedMarker, Origin};

  use super::Markers;

  #[test]
  fn prepare_error() {
    let mut entry = BuildEntry::new("error: test", Origin::default());
    Markers::prepare(&mut entry);
    assert_eq!(
      entry.marker(),
      Some((BuildTagKind::Error, Some(&CapturedMarker::new(0, "error:"))))
    )
  }

  #[test]
  fn prepare_warning() {
    let mut entry = BuildEntry::new("warning: test", Origin::default());
    Markers::prepare(&mut entry);
    assert_eq!(
      entry.marker(),
      Some((
        BuildTagKind::Warning,
        Some(&CapturedMarker::new(0, "warning:"))
      ))
    )
  }

  #[test]
  fn prepare_note() {
    let mut entry = BuildEntry::new("note: test", Origin::default());
    Markers::prepare(&mut entry);
    assert_eq!(
      entry.marker(),
      Some((BuildTagKind::Note, Some(&CapturedMarker::new(0, "note:"))))
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
        selected: None
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
    markers.select(1);
    assert_eq!(markers.selected, Some(1));
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
    assert_eq!(markers.select_next(), 0);
    // second time goes from Some(0) -> Some(1)
    assert_eq!(markers.select_next(), 1);
    // third time goes from Some(1) -> Some(1) as it is out-of-bounds
    assert_eq!(markers.select_next(), 1);
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
    assert_eq!(markers.select_previous(), 0);
    // second time goes from Some(0) -> Some(0)
    assert_eq!(markers.select_previous(), 0);
  }
}
