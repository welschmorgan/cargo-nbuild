use std::{
  fmt::Display,
  io,
  ops::{Deref, DerefMut, Range},
  path::{Path, PathBuf},
  process::{Child, Command, ExitStatus, Stdio},
  str::FromStr,
  sync::{
    mpsc::{channel, Receiver},
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
use regex::Regex;

use crate::{
  dbg, err, CapturedMarker, Debug, DeclaredMarker, Error, ErrorKind, MarkerRef, Markers,
  TryLockFor, BUILD_MARKERS,
};

/// Represent the `cargo build` process.
pub struct BuildCommand(Child);

impl BuildCommand {
  /// Spawn the process, setting piped stdout/stderr streams
  pub fn spawn(args: Vec<String>) -> io::Result<Self> {
    let child = Command::new("cargo")
      .arg("build")
      .args(args)
      .stderr(Stdio::piped())
      .stdout(Stdio::piped())
      .spawn()?;

    Ok(BuildCommand(child))
  }
}

impl Deref for BuildCommand {
  type Target = Child;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for BuildCommand {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

/// Represent a stream origin: either [`std::io::Stdout`] or [`std::io::Stderr`]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum Origin {
  Stdout,
  Stderr,
}

impl Default for Origin {
  fn default() -> Self {
    Self::Stdout
  }
}

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
impl Location {
  pub fn new<P: AsRef<Path>>(path: P, line: Option<usize>, column: Option<usize>) -> Self {
    Self {
      path: path.as_ref().to_path_buf(),
      line,
      column,
    }
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
      location: Some(Location {
        path: path.as_ref().to_path_buf(),
        line,
        column,
      }),
    }
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

  /// Define a [`BuildTag`]
  pub fn set_tag(&mut self, t: BuildTag) {
    if let Some(tag) = self.tag_mut(t.kind) {
      *tag = t;
    } else {
      self.tags.push(t);
    }
  }

  /// Retrieve a mutable ref to an existing [`BuildTag`]
  pub fn tag_mut(&mut self, t: BuildTagKind) -> Option<&mut BuildTag> {
    self.tags.iter_mut().find(|cur| cur.kind == t)
  }

  /// Retrieve a ref to an existing [`BuildTag`]
  pub fn tag(&self, t: BuildTagKind) -> Option<&BuildTag> {
    self.tags.iter().find(|cur| cur.kind == t)
  }

  /// Retrieve any [`Marker`] associated to this tag
  pub fn marker(&self) -> Option<MarkerRef> {
    for known_marker in BUILD_MARKERS.iter() {
      if let Some(tag) = self.tag(known_marker.tag) {
        return Some(MarkerRef::new(tag.kind, tag.marker.as_ref(), known_marker));
      }
    }
    return None;
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
      if t.kind == k {
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
        match t.location.as_ref() {
          Some(Location { path, line, column }) => format!(
            "{}{}{}",
            path.display(),
            match line {
              Some(line) => format!(":{}", line),
              None => String::new(),
            },
            match column {
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

  pub fn range(&self) -> &Range<usize> {
    &self.range
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
/// build.push(BuildEntry::new("my log", Origin::Stdout));
/// build.prepare();
/// let _lines = build.display();
/// ```
pub struct BuildOutput<'a> {
  /// The raw unprocessed entries
  entries: Vec<BuildEntry>,
  warnings: Vec<usize>,
  errors: Vec<usize>,
  remove_noise: bool,
  cursor: usize,
  prepared: Vec<Line<'a>>,
  markers: Markers,
}

impl<'a> Default for BuildOutput<'a> {
  fn default() -> Self {
    Self {
      entries: Default::default(),
      warnings: Default::default(),
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

  /// Add a new build entry to the unprocessed queue
  pub fn push(&mut self, e: BuildEntry) {
    self.entries.push(e);
  }

  /// Add multiple build entries to the unprocessed queue
  pub fn extend<I: IntoIterator<Item = BuildEntry>>(&mut self, entries: I) {
    self.entries.extend(entries);
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
      let marker_after = chunks.get(1).map(|(id, tag)| *id);
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
      let marker = self.entries[range.start].marker().unwrap();
      let entries = self.entries[range.start..range.end]
        .iter()
        .collect::<Vec<_>>();
      return Some(MarkedBlock::new(marker, range, entries));
    }
    None
  }

  /// Prepare the entries that have not been processed yet
  /// by batch processing in multiple threads.
  pub fn prepare(&mut self) {
    let mut threads = vec![];
    let start_time = Instant::now();
    let mut num_prepared = 0;
    let mut recv = vec![];

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
        threads.push(spawn(move || {
          Debug::log(format!(
            "preparing batch #{} -> {} entries",
            batch_id,
            batch.len()
          ));
          let mut ret: Vec<PreparedEntry<'_>> = vec![];
          for (_, entry) in &mut batch {
            Markers::prepare(entry);
          }
          let margin_width = batch
            .iter()
            .map(|(_id, entry)| {
              if let Some(marker) = entry.marker() {
                return marker.captured().unwrap().text.len();
              }
              return 0;
            })
            .max();
          for (batch_entry_id, (global_entry_id, entry)) in batch.into_iter().enumerate() {
            let mut line = Line::default(); //format!("{} | {}", entry_id, entry.message().to_string());
            let mut margin = Span::default();
            let mut message = entry.message.clone();
            if let Some(marker) = entry.marker() {
              dbg!("entry #{} is a marker: {}", global_entry_id, marker.kind());
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
      dbg!("prepare_mt: all workers done, receiving data ...");
      self
        .prepared
        .resize(self.prepared.len() + num_prepared, Line::default());
      self.cursor += num_prepared;
      for r in recv {
        if let Ok((batch_id, batch)) = r.try_recv() {
          dbg!(
            "prepare_mt: worker #{} produced {} lines",
            batch_id,
            batch.len()
          );
          for entry in batch {
            self.entries[entry.entry_id] = entry.entry;
            self.prepared[entry.entry_id] = entry.display;
          }
        }
      }
      *self.markers.tags_mut() = Markers::from(self.entries.as_slice()).tags().clone();
      if let Ok(g) = locations.lock() {
        for (entry_id, location) in g.iter() {
          let block = self.block_at(*entry_id);
          if let Some(block) = block {
            for i in block.range {
              self.entries[i].set_tag(BuildTag::location(
                location.path.clone(),
                location.line,
                location.column,
              ))
            }
          }
        }
      }
      dbg!(
        "prepare_mt: done preparing {} entries in {}s (selected marker: {:?})",
        num_prepared,
        (Instant::now() - start_time).as_secs_f32(),
        self.markers.selected()
      );
    }
  }

  /// Retrieve the displayable lines
  pub fn display(&self) -> Vec<Line<'_>> {
    let mut ret = self.prepared.clone();
    if let Some(sel_entry_id) = self.markers.selected_entry() {
      ret[sel_entry_id].style = ret[sel_entry_id]
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

  /// Retrieve the preparation cursor.
  /// This value corresponds to the number of [`BuildEntry`] we
  /// already processed.
  /// It gets updated in each [`BuildOutput::prepare`] call
  pub fn cursor(&self) -> usize {
    self.cursor
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

/// Represent a cargo build event
#[derive(Debug, Clone, Copy)]
pub enum BuildEvent {
  /// Cargo process spawned
  BuildStarted,
  /// Cargo process finished
  BuildFinished(ExitStatus),
}

#[cfg(test)]
mod tests {
  use std::ops::Range;

  use crate::{
    must_know_marker, BuildEntry, BuildTag, BuildTagKind, CapturedMarker, MarkedBlock, MarkerRef,
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
          BuildTag::warning(0..8, "warning:"),
          BuildTag::location("src/lib\\build.rs", Some(450), Some(7))
        ])
        .with_created_at(unprepared[0].created_at)
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
        MarkerRef::known(
          BuildTagKind::Warning,
          Some(&CapturedMarker::new(0, "warning:"))
        ),
        0..4,
        build.entries[0..4].iter().collect::<Vec<_>>(),
      ))
    );
    assert_eq!(
      build.block_at(5),
      Some(MarkedBlock::new(
        MarkerRef::known(BuildTagKind::Error, Some(&CapturedMarker::new(4, "error:"))),
        4..7,
        build.entries[4..7].iter().collect::<Vec<_>>(),
      ))
    );
  }
}
