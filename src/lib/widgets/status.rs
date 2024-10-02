use std::{fmt::Display, process::ExitStatus};

use ratatui::{
  style::{Style, Stylize},
  text::{Line, Span},
  widgets::{Paragraph, Widget},
};

use crate::{BuildEvent};

#[derive(Clone, Copy, Debug)]
pub struct StatusPart([u8; STATUS_MSG_LEN], usize, Style);

impl Default for StatusPart {
  fn default() -> Self {
    Self([0; STATUS_MSG_LEN], 0, Default::default())
  }
}

impl From<(&str, Style)> for StatusPart {
  fn from(value: (&str, Style)) -> Self {
    let mut ret = [0 as u8; STATUS_MSG_LEN];
    let bytes = value.0.as_bytes();
    let mut len = 0;
    if bytes.len() > 0 {
      for i in 0..bytes.len() {
        if i >= STATUS_MSG_LEN {
          break;
        }
        len = i;
        ret[i] = bytes[i];
      }
      len += 1;
    }
    StatusPart(ret, len, value.1)
  }
}

pub struct StatusIter<'a> {
  parts: &'a [StatusPart; STATUS_MSG_PARTS],
  cursor: usize,
  len: usize,
}

impl<'a> Iterator for StatusIter<'a> {
  type Item = &'a StatusPart;

  fn next(&mut self) -> Option<Self::Item> {
    if self.cursor < self.len {
      let ret = &self.parts[self.cursor];
      self.cursor += 1;
      return Some(ret);
    }
    None
  }
}

#[derive(Clone, Copy, Debug)]
pub struct StatusMessage {
  parts: [StatusPart; STATUS_MSG_PARTS],
  len: usize,
}

impl StatusMessage {
  pub fn new<'a, S: AsRef<str>, I: IntoIterator<Item = (S, Style)>>(s: I) -> Self {
    let byte_parts = s
      .into_iter()
      .map(|(part, style)| StatusPart::from((part.as_ref(), style)))
      .collect::<Vec<_>>();
    let mut parts = [StatusPart::default(); STATUS_MSG_PARTS];
    let mut len = 0;
    if !byte_parts.is_empty() {
      for i in 0..byte_parts.len() {
        if i >= STATUS_MSG_PARTS {
          break;
        }
        parts[i] = byte_parts[i];
        len = i;
      }
      len += 1;
    }
    Self { parts, len }
  }

  pub fn iter<'a>(&'a self) -> StatusIter<'a> {
    StatusIter {
      parts: &self.parts,
      cursor: 0,
      len: self.len,
    }
  }

  pub fn spans(&self) -> Vec<Span> {
    self
      .iter()
      .map(|part| match std::str::from_utf8(&part.0[0..part.1]) {
        Ok(msg) => Some(Span::styled(msg, part.2)),
        Err(e) => {
          crate::dbg!("failed to decode utf-8, {}", e);
          None
        }
      })
      .filter_map(|part| {
        if part.is_some() {
          return Some(part.unwrap());
        }
        None
      })
      .collect::<Vec<_>>()
  }
}

impl Default for StatusMessage {
  fn default() -> Self {
    Self {
      parts: [StatusPart::default(); STATUS_MSG_PARTS],
      len: 0,
    }
  }
}

impl Display for StatusMessage {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let parts = self.parts[0..self.len]
      .iter()
      .map(|part| unsafe { std::str::from_utf8_unchecked(&part.0[0..part.1]) })
      .collect::<Vec<_>>();
    write!(
      f,
      "StatusMessage([{}])",
      match parts.len() {
        0 => String::new(),
        _ => format!("\"{}\"", parts.join("\", \"")),
      }
    )
  }
}

pub const STATUS_BAR_BUF: usize = 15;
pub const STATUS_MSG_PARTS: usize = 10;
pub const STATUS_MSG_LEN: usize = 100;

/// The status bar widget displays various informations about
/// the current build status.
#[derive(Default, Clone, Copy)]
pub struct StatusBar {
  messages: [Option<StatusMessage>; STATUS_BAR_BUF],
  cursor: usize,
  num_errors: usize,
  num_warnings: usize,
  num_notes: usize,
  num_output_lines: usize,
  num_prepared_lines: usize,
}

impl StatusBar {
  /// Set the build event to be displayed
  pub fn with_event(mut self, evt: BuildEvent) -> Self {
    if let Some(msg) = self.transform(&evt) {
      if self.cursor < self.messages.len().saturating_sub(1) {
        self.messages[self.cursor] = Some(msg);
        self.cursor += 1;
      } else {
        self.messages.rotate_right(1);
        self.messages[self.cursor] = Some(msg);
      }
    }
    self
  }

  pub fn with_message(mut self, msg: StatusMessage) -> Self {
    self.push_message(msg);
    self
  }

  pub fn push_message(&mut self, message: StatusMessage) {
    self.messages.rotate_right(1);
    self.messages[self.cursor] = Some(message);
    if self.cursor < self.messages.len().saturating_sub(1) {
      self.cursor += 1;
    }
  }

  /// Set the number of errors
  pub fn with_num_errors(mut self, n: usize) -> Self {
    self.num_errors = n;
    self
  }

  /// Set the number of notes
  pub fn with_num_notes(mut self, n: usize) -> Self {
    self.num_notes = n;
    self
  }

  /// Set the number of warnings
  pub fn with_num_warnings(mut self, n: usize) -> Self {
    self.num_warnings = n;
    self
  }

  /// Set the number of output lines
  pub fn with_num_output_lines(mut self, n: usize) -> Self {
    self.num_output_lines = n;
    self
  }

  /// Set the number of processed output lines
  pub fn with_num_prepared_lines(mut self, n: usize) -> Self {
    self.num_prepared_lines = n;
    self
  }

  fn transform(&self, evt: &BuildEvent) -> Option<StatusMessage> {
    match evt {
      BuildEvent::BuildError(_) => None,
      BuildEvent::BuildFinished(status) => Some(self.transform_build_finished(*status)),
      BuildEvent::BuildStarted => Some(self.transform_build_started()),
    }
  }

  fn transform_build_started(&self) -> StatusMessage {
    StatusMessage::new([
      ("Build ", Style::default()),
      ("running", Style::default().gray()),
      ("⌛", Style::default()),
    ])
  }

  fn transform_build_finished(&self, exit: ExitStatus) -> StatusMessage {
    StatusMessage::new([
      ("Build ".to_string(), Style::default()),
      ("finished".to_string(), Style::default().bold()),
      match exit.success() {
        true => (" ✓".to_string(), Style::default().bold().green()),
        false => (" ✗".to_string(), Style::default().bold().red()),
      },
      (" | ".to_string(), Style::default()),
      match exit.success() {
        true => (format!("{}", exit), Style::default().dim()),
        false => (format!("{}", exit), Style::default()),
      },
      (" | ".to_string(), Style::default()),
      match self.num_errors {
        0 => ("no errors".to_string(), Style::default().dim()),
        _ => (
          format!("{} error(s)", self.num_errors),
          Style::default().red(),
        ),
      },
      (" | ".to_string(), Style::default()),
      match self.num_warnings {
        0 => ("no warnings".to_string(), Style::default().dim()),
        _ => (
          format!("{} warning(s)", self.num_warnings),
          Style::default().yellow(),
        ),
      },
      (" | ".to_string(), Style::default()),
      match self.num_notes {
        0 => ("no notes".to_string(), Style::default().dim()),
        _ => (
          format!("{} notes(s)", self.num_notes),
          Style::default().blue(),
        ),
      },
      (" | ".to_string(), Style::default()),
      match self.num_prepared_lines == self.num_output_lines {
        true => (
          format!("{} line(s)", self.num_output_lines),
          Style::default().dim(),
        ),
        false => (
          format!(
            "{}/{} line(s) prepared",
            self.num_prepared_lines, self.num_output_lines
          ),
          Style::default(),
        ),
      },
    ])
  }

  pub fn last_message(&self) -> Option<&StatusMessage> {
    for i in (0..=self.cursor).into_iter().rev() {
      if self.messages[i].is_some() {
        return self.messages[i].as_ref();
      }
    }
    return None;
  }
}

impl Widget for StatusBar {
  fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
  where
    Self: Sized,
  {
    if let Some(msg) = self.last_message() {
      let para = Paragraph::new(Line::default().spans(msg.spans()));
      para.render(area, buf);
    }
  }
}
