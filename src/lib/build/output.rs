use std::{
  ops::Range,
  sync::{
    mpsc::{channel, Receiver, Sender},
    Arc, Mutex,
  },
  thread::spawn,
  time::{Duration, Instant},
};

use lazy_static::lazy_static;
use ratatui::{
  style::{Style, Stylize},
  text::{Line, Span},
};

use crate::{
  err, BuildTagKind, Debug, DeclaredMarker, ErrorKind, LogEntry, MarkerSelection, Markers,
  TryLockFor, DEFAULT_RULES,
};

use super::{active_rule, BuildEntry, BuildEvent, BuildTag, Location, MarkedBlock, Rule};

/// The BuildOutput struct prepares the [`BuildCommand`] raw output lines.
/// It creates the necessary [`ratatui`] elements: [`Line`] and [`Span`]
/// to be rendered later by the [`crate::widgets::log::LogView`] widget.
///
/// It first aggregates the raw entries in [`BuildOutput::entries`],
/// and when [`BuildOutput::prepare`] gets called it batches up those unprocessed
/// entries into the [`BuildOutput::prepared`] vector.
///
/// After calling [`BuildOutput::prepare`], you can get the final lines to
/// be displayed by calling [`BuildOutput::display`].
///
/// # Examples
///
/// ```
/// use cargo_nbuild::{BuildOutput, BuildEntry, Origin};
///
/// let mut build = BuildOutput::default();
/// let (tx, rx) = std::sync::mpsc::channel();
/// build.push(BuildEntry::new("my log", Origin::Stdout));
/// build.prepare(tx);
/// let _lines = build.display();
/// ```
pub struct BuildOutput<'a> {
  rule: Rule,
  /// The raw unprocessed entries
  entries: Vec<BuildEntry>,
  warnings: Vec<usize>,
  notes: Vec<usize>,
  errors: Vec<usize>,
  build_events: Option<Sender<BuildEvent>>,
  remove_noise: bool,
  cursor: usize,
  prepared: Vec<Line<'a>>,
  markers: Markers,
}

impl<'a> Default for BuildOutput<'a> {
  fn default() -> Self {
    Self {
      rule: active_rule(),
      entries: Default::default(),
      warnings: Default::default(),
      notes: Default::default(),
      build_events: Default::default(),
      errors: Default::default(),
      remove_noise: Default::default(),
      cursor: Default::default(),
      prepared: Default::default(),
      markers: Default::default(),
    }
  }
}

impl<'a> BuildOutput<'a> {
  /// Maximum number of items processed per [`Self::prepare`] call
  pub const MAX_PREPARE_BATCH_SIZE: usize = 5;

  /// Base capacity of prepared lines [`Self::prepare`] call
  pub const BASE_PREPARED_CAPACITY: usize = 500;

  /// Number of workers to spawn for display preparation [`Self::prepare`] call
  pub const WORKERS: u8 = 5;

  /// If true remove non-marker output lines
  pub fn with_noise_removed(mut self, r: bool) -> Self {
    self.remove_noise = r;
    self
  }

  /// If true remove non-marker output lines
  pub fn with_build_events(mut self, events: Sender<BuildEvent>) -> Self {
    self.build_events = Some(events);
    self
  }

  /// Add a new build entry to the unprocessed queue
  pub fn push(&mut self, e: BuildEntry) {
    self.entries.push(e);
  }

  /// Add multiple build entries to the unprocessed queue
  pub fn extend<Item: Into<BuildEntry>, Iter: IntoIterator<Item = Item>>(&mut self, entries: Iter) {
    self
      .entries
      .extend(entries.into_iter().map(|e| e.into()).collect::<Vec<_>>());
  }

  /// Pull all build entries from the supplied [`Receiver`].
  ///
  /// Does not block the current thread
  pub fn pull(&mut self, from: &Receiver<Vec<BuildEntry>>) {
    while let Ok(entries) = from.try_recv() {
      self.entries.extend(entries);
    }
  }

  /// Tag a [`BuildEntry`] with the supplied [`BuildTag`]
  pub fn tag_entry(&mut self, i: usize, tag: BuildTag) {
    if let Some(e) = self.entries.get_mut(i) {
      e.tags_mut().push(tag);
    }
  }

  /// Detect if there is any entry
  pub fn has_any_entries(&self) -> bool {
    self.num_entries() > 0
  }

  /// Detect if there is unprepared entries to be prepared
  pub fn has_unprepared_entries(&self) -> bool {
    self.cursor < self.entries.len()
  }

  /// Retrieve the number of entries stored
  pub fn num_entries(&self) -> usize {
    self.entries.len()
  }

  /// Retrieve the unprepared entries tuple `(id, entry)`
  pub fn unprepared_entries(&self) -> Option<Vec<(usize, &BuildEntry)>> {
    if self.cursor >= self.entries.len() {
      return None;
    }
    Some(
      self
        .entries
        .iter()
        .enumerate()
        .skip(self.cursor)
        .map(|(id, item)| (id, item))
        .collect::<Vec<_>>(),
    )
  }

  /// Batch the unprepared entries corresponding to the [`Self::WORKERS`]
  pub fn batch_unprepared_entries(&self) -> Option<Vec<(usize, Vec<(usize, BuildEntry)>)>> {
    match self.unprepared_entries() {
      None => None,
      Some(entries) => {
        let mut ret = vec![];
        for (batch_id, batch) in entries
          .chunks(self.entries.len().max(Self::WORKERS as usize) / Self::WORKERS as usize)
          .enumerate()
        {
          let mut rbatch = vec![];
          for (entry_id, entry) in batch.iter() {
            rbatch.push((*entry_id, (*entry).clone()));
          }
          ret.push((batch_id, rbatch));
        }
        Some(ret)
      }
    }
  }

  /// Retrieve the list of markers
  pub fn markers(&self) -> &Markers {
    &self.markers
  }

  /// Retrieve the list of markers as a mutable reference
  pub fn markers_mut(&mut self) -> &mut Markers {
    &mut self.markers
  }

  pub fn extract_location<M: AsRef<str>>(message: M) -> crate::Result<Option<Location>> {
    let trimmed_message = message.as_ref().trim();
    if trimmed_message.starts_with("-->") {
      return Ok(Some(trimmed_message[3..].trim().parse::<Location>()?));
    }
    Ok(None)
  }

  pub fn block_range_at(&self, entry_id: usize) -> Option<Range<usize>> {
    if self.markers.is_empty() {
      return None;
    }
    let mut found = None;
    for chunks in self.markers.chunks(2) {
      let (marker_before, _) = chunks[0];
      let marker_after = chunks.get(1).map(|(id, _tag)| *id);
      if entry_id >= marker_before && (marker_after.is_none() || entry_id < marker_after.unwrap()) {
        found = Some((marker_before, marker_after));
        break;
      }
    }
    found
      .or_else(|| Some((self.markers.last().unwrap().0, None)))
      .map(|(before, after)| Range {
        start: before,
        end: after.unwrap_or_else(|| self.entries.len()),
      })
  }

  pub fn block_at(&'a self, entry_id: usize) -> Option<MarkedBlock<'a>> {
    if let Some(range) = self.block_range_at(entry_id) {
      let marker_id = *self
        .markers
        .iter()
        .enumerate()
        .find_map(|(marker_id, (entry_id, _tag))| {
          if *entry_id == range.start {
            return Some(marker_id);
          }
          None
        })
        .as_ref()
        .expect("block found with no coresponding marker");
      let marker = self.entries[range.start].first_marker().unwrap();
      let entries = self.entries[range.start..range.end]
        .iter()
        .collect::<Vec<_>>();
      return Some(MarkedBlock::new(marker_id, marker.clone(), range, entries));
    }
    None
  }

  pub fn block_size(&'a self, entry_id: usize) -> Option<usize> {
    if let Some(block) = self.block_at(entry_id) {
      return Some(block.entries().len());
    }
    None
  }

  pub fn send_event(&self, event: BuildEvent) {
    if let Some(events) = &self.build_events {
      let _ = events.send(event);
    }
  }

  pub fn find_marker(&self, tag: BuildTagKind) -> Option<&DeclaredMarker> {
    for marker in &self.rule.markers {
      if tag == marker.tag {
        return Some(marker);
      }
    }
    None
  }

  /// Prepare the entries that have not been processed yet
  /// by batch processing in multiple threads.
  pub fn prepare(&mut self) -> bool {
    let mut threads = vec![];
    let start_time = Instant::now();
    let mut num_prepared = 0;
    let mut recv = vec![];

    #[allow(unused)]
    struct PreparedEntry<'a> {
      pub batch_id: usize,
      pub entry_id: usize,
      pub entry: BuildEntry,
      pub display: Line<'a>,
    }

    let locations: Arc<Mutex<Vec<(usize, Location)>>> = Arc::new(Mutex::new(Vec::new()));

    if let Some(batches) = self.batch_unprepared_entries() {
      for (batch_id, mut batch) in batches {
        num_prepared += batch.len();
        let (tx, rx) = channel::<(usize, Vec<PreparedEntry<'_>>)>();
        recv.push(rx);
        let style_log = Style::default().dim();
        let th_locations = locations.clone();
        let rule = self.rule.clone();
        threads.push(spawn(move || {
          Debug::log(format!(
            "preparing batch #{} -> {} entries",
            batch_id,
            batch.len()
          ));
          let mut ret: Vec<PreparedEntry<'_>> = vec![];
          for (_, entry) in &mut batch {
            if let Err(e) = Markers::prepare(entry, &rule) {
              crate::dbg!("Failed to prepare markers: {}", e);
            }
          }
          let margin_width = batch
            .iter()
            .map(|(_id, entry)| {
              if let Some(marker) = entry.first_marker() {
                return marker.captured().unwrap().text.len();
              }
              return 0;
            })
            .max();
          for (_batch_entry_id, (global_entry_id, entry)) in batch.into_iter().enumerate() {
            let mut line = Line::default(); //format!("{} | {}", entry_id, entry.message().to_string());
            let mut margin = Span::default();
            let mut message = entry.message().clone();

            if let Some(marker) = entry.first_marker() {
              // crate::dbg!("entry #{} is a marker: {}", global_entry_id, marker.kind());
              let captured = marker.captured().unwrap();
              margin = margin.content(captured.text.clone());
              margin = margin.style(marker.declared().style);
              message = message.as_str()[captured.range.end..].to_string();
            } else {
              if let Ok(Some(loc)) = Self::extract_location(message.as_str()) {
                if let Ok(mut g) = th_locations.try_lock_for(Duration::from_millis(150)) {
                  g.push((global_entry_id, loc));
                }
              }
              margin = margin.content(" ".repeat(margin_width.unwrap_or_else(|| 4)));
              margin = margin.style(style_log);
            }
            line.push_span(margin);
            line.push_span(" ");
            line.push_span(message);
            ret.push(PreparedEntry {
              batch_id,
              entry_id: global_entry_id,
              entry,
              display: line,
            });
          }
          let _ = tx.send((batch_id, ret));
        }));
      }
      for th in threads {
        let _ = th.join();
      }
      crate::dbg!("prepare_mt: all workers done, receiving data ...");
      self
        .prepared
        .resize(self.prepared.len() + num_prepared, Line::default());
      self.cursor += num_prepared;
      let mut selection = None;
      for r in recv {
        if let Ok((batch_id, batch)) = r.try_recv() {
          crate::dbg!(
            "prepare_mt: worker #{} produced {} lines",
            batch_id,
            batch.len()
          );
          for entry in batch {
            if let Some(_) = entry.entry.tag(BuildTagKind::Error) {
              self.send_event(BuildEvent::BuildError(entry.entry_id));
              if self.markers.selection().is_none() {
                selection = Some(entry.entry_id);
              }
              self.errors.push(entry.entry_id);
            }
            if let Some(_) = entry.entry.tag(BuildTagKind::Warning) {
              self.warnings.push(entry.entry_id);
            }
            if let Some(_) = entry.entry.tag(BuildTagKind::Note) {
              self.notes.push(entry.entry_id);
            }
            self.entries[entry.entry_id] = entry.entry;
            self.prepared[entry.entry_id] = entry.display;
          }
        }
      }
      *self.markers.tags_mut() = Markers::from_entries(self.entries.as_slice())
        .tags()
        .clone();
      if let Some(sel) = selection {
        self.select_block_from_entry(sel);
      }
      if let Ok(g) = locations.lock() {
        for (entry_id, location) in g.iter() {
          let block = self.block_at(*entry_id);
          if let Some(block) = block {
            for i in block.range() {
              self.entries[i].set_tag(BuildTag::location(
                location.path().clone(),
                location.line(),
                location.column(),
              ))
            }
          }
        }
      }
      crate::dbg!(
        "prepare_mt: done preparing {} entries in {}s (selected marker: {:?})",
        num_prepared,
        (Instant::now() - start_time).as_secs_f32(),
        self.markers.selection()
      );
      return true;
    }
    false
  }

  pub fn select_block_from_entry(&mut self, entry_id: usize) {
    let marker_id = match self.block_at(entry_id) {
      Some(block) => block.marker_id(),
      None => 0,
    };
    self.markers.select(marker_id, None);
  }

  pub fn select_entry(&mut self, entry_id: usize, region: Option<Range<usize>>) {
    let marker_id = match self.block_at(entry_id) {
      Some(block) => block.marker_id(),
      None => 0,
    };
    self
      .markers
      .set_selection(Some(MarkerSelection::new(marker_id, entry_id, region)));
    crate::dbg!(
      "Selecting marker #{} -> {:?}",
      marker_id,
      self.markers.selection()
    );
  }

  /// Retrieve the displayable lines
  pub fn display(&self) -> Vec<LogEntry<'_>> {
    let mut ret = self
      .prepared
      .iter()
      .enumerate()
      .map(|(id, line)| LogEntry::new(line.clone(), self.entries[id].tags().clone()))
      .collect::<Vec<_>>();
    if let Some(sel_entry_id) = self.markers.selected_entry() {
      ret[sel_entry_id].line_mut().style = ret[sel_entry_id]
        .line()
        .style
        .patch(Style::default().on_light_blue());
    }
    ret
  }

  /// Retrieve the stored entries
  pub fn entries(&self) -> &Vec<BuildEntry> {
    &self.entries
  }

  /// Retrieve the detected errors
  pub fn errors(&self) -> &Vec<usize> {
    &self.errors
  }

  /// Retrieve the detected warnings
  pub fn warnings(&self) -> &Vec<usize> {
    &self.warnings
  }

  /// Retrieve the detected notes
  pub fn notes(&self) -> &Vec<usize> {
    &self.notes
  }

  /// Retrieve the preparation cursor.
  /// This value corresponds to the number of [`BuildEntry`] we
  /// already processed.
  /// It gets updated in each [`BuildOutput::prepare`] call
  pub fn cursor(&self) -> usize {
    self.cursor
  }

  pub fn search<Q: AsRef<str>>(&self, query: Q) -> Option<(MarkedBlock, MarkerSelection)> {
    self
      .entries
      .iter()
      .enumerate()
      .find_map(|(entry_id, entry)| {
        if let Some(pos) = entry.message().find(query.as_ref()) {
          let block = self.block_at(entry_id).unwrap();
          let marker_id = block.marker_id();
          return Some((
            block,
            MarkerSelection::new(marker_id, entry_id, Some(pos..pos + query.as_ref().len())),
          ));
        }
        None
      })
  }
}

impl<'a, T: Into<BuildEntry>, I: IntoIterator<Item = T>> From<I> for BuildOutput<'a> {
  fn from(value: I) -> Self {
    let mut ret = BuildOutput::default();
    ret.extend(
      value
        .into_iter()
        .map(|item| item.into())
        .collect::<Vec<_>>(),
    );
    ret
  }
}

#[cfg(test)]
mod tests {
  use std::{ops::Range, sync::mpsc::channel};

  use crate::{
    BuildEntry, BuildTag, BuildTagKind, CapturedMarker, MarkedBlock, MarkerRef, MarkerSelection,
    Origin,
  };

  use super::BuildOutput;

  #[test]
  fn prepare() {
    let sample_output = r#"warning: field `batch_id` is never read
   --> src/lib\build.rs:450:7
    |
449 |     struct PreparedEntry<'a> {
    |            ------------- field in this struct
450 |       batch_id: usize,
    |       ^^^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default"#;
    let mut build = BuildOutput::from(sample_output.split('\n')).with_noise_removed(false);
    build.prepare();
    let unprepared = build.entries();
    let lines = build.display();
    assert_eq!(unprepared.len(), lines.len());
    assert_eq!(
      unprepared[0],
      BuildEntry::new("warning: field `batch_id` is never read", Origin::default())
        .with_tags(vec![
          BuildTag::warning(0..8, "warning:").expect("invalid warning marker"),
          BuildTag::location("src/lib\\build.rs", Some(450), Some(7))
        ])
        .with_created_at(unprepared[0].created_at().clone())
    );
  }

  #[test]
  fn block_range_at() {
    let sample_output = r#"warning: field `batch_id` is never read
    blasdf
    asdf asdf asdf
    adsf s
    error: test error
    blasdf asdfl alsdf
    asdfasdf"#;
    let mut build = BuildOutput::from(sample_output.split('\n')).with_noise_removed(false);
    build.prepare();
    assert_eq!(build.block_range_at(1), Some(Range { start: 0, end: 4 }));
    assert_eq!(build.block_range_at(5), Some(Range { start: 4, end: 7 }));
  }

  #[test]
  fn block_at() {
    let sample_output = r#"warning: field `batch_id` is never read
    blasdf
    asdf asdf asdf
    adsf s
    error: test error
    blasdf asdfl alsdf
    asdfasdf"#;
    let mut build = BuildOutput::from(sample_output.split('\n')).with_noise_removed(false);
    build.prepare();
    assert_eq!(
      build.block_at(1),
      Some(MarkedBlock::new(
        0,
        MarkerRef::known(
          BuildTagKind::Warning,
          Some(CapturedMarker::new(0, "warning:"))
        ),
        0..4,
        build.entries[0..4].iter().collect::<Vec<_>>(),
      ))
    );
    assert_eq!(
      build.block_at(5),
      Some(MarkedBlock::new(
        1,
        MarkerRef::known(BuildTagKind::Error, Some(CapturedMarker::new(4, "error:"))),
        4..7,
        build.entries[4..7].iter().collect::<Vec<_>>(),
      ))
    );
  }

  #[test]
  fn search() {
    let content = include_str!("../../../samples/rust/rust-panic.log")
      .split("\n")
      .collect::<Vec<_>>();
    let mut build = BuildOutput::default();
    build.extend(content);
    build.prepare();
    assert_eq!(
      build.search("panic"),
      Some((
        MarkedBlock::new(
          0,
          MarkerRef::known(BuildTagKind::Error, Some(CapturedMarker::new(0, "error:"))),
          1..7,
          build
            .entries
            .iter()
            .enumerate()
            .filter(|(id, _entry)| *id >= 1 && *id < 7)
            .map(|(_id, entry)| entry)
            .collect::<Vec<_>>()
        ),
        MarkerSelection::new(0, 6, Some(16..21))
      ))
    );
  }
}
