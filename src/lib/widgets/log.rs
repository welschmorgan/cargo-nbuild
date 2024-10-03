use ratatui::{
  style::Stylize,
  text::Line,
  widgets::{
    Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget,
  },
};

use crate::{BuildTag, BuildTagKind};

#[derive(Clone, Default)]
pub struct LogEntry<'a> {
  line: Line<'a>,
  tags: Vec<BuildTag>,
}

impl<'a> LogEntry<'a> {
  pub fn line(&self) -> &Line<'a> {
    &self.line
  }

  pub fn line_mut(&mut self) -> &mut Line<'a> {
    &mut self.line
  }
}

impl<'a> LogEntry<'a> {
  pub fn new(line: Line<'a>, tags: Vec<BuildTag>) -> Self {
    Self { line, tags }
  }
}

/// Support display of build entries
#[derive(Default)]
pub struct LogView<'a> {
  scroll: usize,
  entries: Vec<LogEntry<'a>>,
  filter: Option<BuildTagKind>,
}

impl<'a> LogView<'a> {
  /// Define the scroll bar value
  pub fn with_scroll(mut self, v: usize) -> Self {
    self.scroll = v;
    self
  }

  pub fn with_filter(mut self, f: BuildTagKind) -> Self {
    self.filter = Some(f);
    self
  }

  /// Update the displayed lines
  pub fn with_content(mut self, content: Vec<LogEntry<'a>>) -> Self {
    self.entries = content;
    self
  }

  pub fn set_filter(&mut self, f: Option<BuildTagKind>) {
    self.filter = f;
  }
}

impl<'a> StatefulWidget for LogView<'a> {
  type State = ScrollbarState;

  fn render(
    self,
    area: ratatui::prelude::Rect,
    buf: &mut ratatui::prelude::Buffer,
    state: &mut Self::State,
  ) {
    let lines = if let Some(tag_filter) = self.filter {
      self
        .entries
        .iter()
        .filter_map(|entry| {
          if self.filter.is_none()
            || entry
              .tags
              .iter()
              .find(|tag| tag.get_kind() == tag_filter)
              .is_some()
          {
            return Some(entry.line.clone());
          }
          None
        })
        .collect::<Vec<_>>()
    } else {
      self
        .entries
        .iter()
        .map(|entry| entry.line.clone())
        .collect::<Vec<_>>()
        .clone()
    };
    *state = state.content_length(lines.len());
    let log = Paragraph::new(lines)
      .gray()
      .block(Block::bordered().gray())
      .scroll((self.scroll as u16, 0));
    log.render(area, buf);
    Scrollbar::new(ScrollbarOrientation::VerticalRight)
      .begin_symbol(Some("↑"))
      .end_symbol(Some("↓"))
      .render(area, buf, state)
  }
}
