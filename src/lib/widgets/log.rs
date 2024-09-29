use ratatui::{
  style::Stylize,
  text::Line,
  widgets::{
    Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget,
  },
};

/// Support display of build entries
#[derive(Default)]
pub struct LogView<'a> {
  scroll: usize,
  content: Vec<Line<'a>>,
}

impl<'a> LogView<'a> {
  /// Define the scroll bar value
  pub fn with_scroll(mut self, v: usize) -> Self {
    self.scroll = v;
    self
  }

  /// Update the displayed lines
  pub fn with_content(mut self, content: Vec<Line<'a>>) -> Self {
    self.content = content;
    self
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
    *state = state.content_length(self.content.len());
    let log = Paragraph::new(self.content)
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

// impl<'a> StatefulWidget for LogView<'a> {
//   type State = ListState;

//   fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
//     let log = List::new(self.content)
//       .gray()
//       .block(Block::bordered().gray());
//     Widget::render(log, area, buf);
//   }
// }
