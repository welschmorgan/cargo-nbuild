use std::{
  fmt::Display,
  io,
  ops::{Deref, DerefMut, Range},
  path::PathBuf,
  process::{Child, Command, ExitStatus, Stdio},
  sync::{
    mpsc::{channel, Receiver},
    Arc,
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

use crate::{dbg, Debug};

pub struct BuildCommand(Child);

impl BuildCommand {
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum Origin {
  Stdout,
  Stderr,
}

#[derive(Debug, Clone)]
pub struct CapturedMarker {
  pub range: Range<usize>,
  pub capture: String,
}

#[derive(Debug, Clone)]
pub struct Marker {
  pub tag: BuildTagKind,
  pub regex: Regex,
  pub style: Style,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Location {
  path: PathBuf,
  line: Option<usize>,
  column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Copy)]
pub enum BuildTagKind {
  Warning,
  Error,
  Note,
  Hidden,
  Location,
}

impl Display for BuildTag {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self)
  }
}

#[derive(Debug, Clone)]
pub struct BuildTag {
  kind: BuildTagKind,
  marker: Option<CapturedMarker>,
  location: Option<Location>,
}

impl BuildTag {
  pub fn warning<C: AsRef<str>>(range: Range<usize>, capture: C) -> Self {
    Self {
      kind: BuildTagKind::Warning,
      marker: Some(CapturedMarker {
        range,
        capture: capture.as_ref().to_string(),
      }),
      location: None,
    }
  }

  pub fn error<C: AsRef<str>>(range: Range<usize>, capture: C) -> Self {
    Self {
      kind: BuildTagKind::Error,
      marker: Some(CapturedMarker {
        range,
        capture: capture.as_ref().to_string(),
      }),
      location: None,
    }
  }

  pub fn hidden() -> Self {
    Self {
      kind: BuildTagKind::Hidden,
      marker: None,
      location: None,
    }
  }

  pub fn location<P: AsRef<PathBuf>>(path: P, line: Option<usize>, column: Option<usize>) -> Self {
    Self {
      kind: BuildTagKind::Error,
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

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct BuildEntry {
  created_at: Instant,
  message: String,
  origin: Origin,
  tags: Vec<BuildTag>,
}

impl BuildEntry {
  pub fn new<M: AsRef<str>>(msg: M, orig: Origin) -> Self {
    Self {
      created_at: Instant::now(),
      message: msg.as_ref().to_string(),
      origin: orig,
      tags: vec![],
    }
  }

  pub fn created_at(&self) -> &Instant {
    &self.created_at
  }

  pub fn message(&self) -> &String {
    &self.message
  }

  pub fn origin(&self) -> Origin {
    self.origin
  }

  pub fn set_tag(&mut self, t: BuildTag) {
    if let Some(tag) = self.tag_mut(t.kind) {
      *tag = t;
    } else {
      self.tags.push(t);
    }
  }

  pub fn tag_mut(&mut self, t: BuildTagKind) -> Option<&mut BuildTag> {
    self.tags.iter_mut().find(|cur| cur.kind == t)
  }

  pub fn tag(&self, t: BuildTagKind) -> Option<&BuildTag> {
    self.tags.iter().find(|cur| cur.kind == t)
  }

  pub fn marker(&self) -> Option<(BuildTagKind, Option<&CapturedMarker>)> {
    for known_marker in BUILD_MARKERS.iter() {
      if let Some(tag) = self.tag(known_marker.tag) {
        return Some((tag.kind, tag.marker.as_ref()));
      }
    }
    return None;
  }

  pub fn tags(&self) -> &Vec<BuildTag> {
    &self.tags
  }

  pub fn tags_mut(&mut self) -> &mut Vec<BuildTag> {
    &mut self.tags
  }

  pub fn location(&self) -> Option<&BuildTag> {
    self.tag(BuildTagKind::Location)
  }

  pub fn has_tag(&self, k: BuildTagKind) -> bool {
    for t in &self.tags {
      if t.kind == k {
        return true;
      }
    }
    return false;
  }

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

pub struct BuildOutput<'a> {
  entries: Vec<BuildEntry>,
  warnings: Vec<usize>,
  errors: Vec<usize>,
  remove_noise: bool,
  update_threshold: Option<Duration>,
  cursor: usize,
  prepared: Vec<Line<'a>>,
}

impl<'a> Default for BuildOutput<'a> {
  fn default() -> Self {
    Self {
      entries: Default::default(),
      warnings: Default::default(),
      errors: Default::default(),
      remove_noise: Default::default(),
      update_threshold: Default::default(),
      cursor: 0,
      prepared: vec![],
    }
  }
}

lazy_static! {
  static ref BUILD_MARKERS: Arc<Vec<Marker>> = Arc::new(vec![
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

impl<'a> BuildOutput<'a> {
  /// Maximum number of items processed per [`Self::prepare`] call
  pub const MAX_PREPARE_BATCH_SIZE: usize = 5;

  /// Base capacity of prepared lines [`Self::prepare`] call
  pub const BASE_PREPARED_CAPACITY: usize = 500;

  /// Number of workers to spawn for display preparation [`Self::prepare`] call
  pub const WORKERS: u8 = 5;

  pub fn with_noise_removed(mut self, r: bool) -> Self {
    self.remove_noise = r;
    self
  }

  pub fn with_update_threshold(mut self, d: Duration) -> Self {
    self.update_threshold = Some(d);
    self
  }

  pub fn push(&mut self, e: BuildEntry) {
    self.entries.push(e);
  }

  pub fn pull(&mut self, from: &Receiver<Vec<BuildEntry>>) {
    while let Ok(entries) = from.try_recv() {
      self.entries.extend(entries);
    }
  }

  pub fn tag_entry(&mut self, i: usize, tag: BuildTag) {
    if let Some(e) = self.entries.get_mut(i) {
      e.tags_mut().push(tag);
    }
  }

  pub fn has_any_entries(&self) -> bool {
    self.num_entries() > 0
  }

  pub fn has_unprepared_entries(&self) -> bool {
    self.cursor < self.entries.len()
  }

  pub fn num_entries(&self) -> usize {
    self.entries.len()
  }

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

  fn prepare_markers(entry: &mut BuildEntry) {
    for known_marker in BUILD_MARKERS.iter() {
      if let Some(m) = known_marker.regex.find(&entry.message()) {
        entry.set_tag(BuildTag {
          kind: known_marker.tag,
          marker: Some(CapturedMarker {
            range: m.range(),
            capture: m.as_str().to_string(),
          }),
          location: None,
        });
      }
    }
  }

  pub fn prepare_mt(&mut self) {
    let mut threads = vec![];
    let start_time = Instant::now();
    let mut num_prepared = 0;
    let mut recv = vec![];
    if let Some(batches) = self.batch_unprepared_entries() {
      for (batch_id, mut batch) in batches {
        num_prepared += batch.len();
        let (tx, rx) = channel::<(usize, Vec<(usize, Line<'_>)>)>();
        recv.push(rx);
        let style_log = Style::default().dim();
        threads.push(spawn(move || {
          Debug::log(format!(
            "preparing batch #{} -> {} entries",
            batch_id,
            batch.len()
          ));
          let mut ret = vec![];
          for (_, entry) in &mut batch {
            Self::prepare_markers(entry);
          }
          let margin_width = batch
            .iter()
            .map(|(_id, entry)| {
              if let Some((_kind, captured)) = entry.marker() {
                return captured.as_ref().unwrap().capture.len();
              }
              return 0;
            })
            .max();
          for (entry_id, entry) in batch {
            let mut line = Line::default(); //format!("{} | {}", entry_id, entry.message().to_string());
            let mut margin = Span::default();
            let mut message = entry.message.clone();
            let mut found_marker = false;
            for known_marker in BUILD_MARKERS.iter() {
              if let Some(tag) = entry.tag(known_marker.tag) {
                let captured = tag.marker.as_ref().unwrap();
                margin = margin.content(captured.capture.clone());
                margin = margin.style(known_marker.style);
                message = message.as_str()[captured.range.end..].to_string();
                found_marker = true;
                break;
              }
            }
            if !found_marker {
              margin = margin.content(" ".repeat(margin_width.unwrap_or_else(|| 4)));
              margin = margin.style(style_log);
            }
            line.push_span(margin);
            line.push_span(" ");
            line.push_span(message);
            ret.push((entry_id, line));
          }
          tx.send((batch_id, ret))
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
          for (id, entry) in batch {
            self.prepared[id] = entry;
          }
        }
      }
      dbg!(
        "prepare_mt: done preparing {} entries in {}s",
        num_prepared,
        (Instant::now() - start_time).as_secs_f32()
      );
    }
  }

  pub fn display(&self) -> Vec<Line<'_>> {
    return self.prepared.clone();
  }

  pub fn entries(&self) -> &Vec<BuildEntry> {
    &self.entries
  }
  pub fn errors(&self) -> &Vec<usize> {
    &self.errors
  }
  pub fn warnings(&self) -> &Vec<usize> {
    &self.warnings
  }

  pub fn cursor(&self) -> usize {
    self.cursor
  }
}

#[derive(Debug, Clone, Copy)]
pub enum BuildEvent {
  BuildStarted,
  BuildFinished(ExitStatus),
}
