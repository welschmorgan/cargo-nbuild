use ratatui::{
  crossterm::event::{KeyCode, KeyEvent},
  layout::{Alignment, Constraint, Flex, Layout, Rect},
  style::Stylize,
  text::Line,
  widgets::{
    Block, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
    Widget,
  },
};

/// The help menu displayed as a popup
pub struct HelpMenu {
  keys: Vec<[String; 2]>,
  scroll: usize,
}

impl HelpMenu {
  /// Construct this object
  pub fn new() -> Self {
    Self {
      keys: vec![],
      scroll: 0,
    }
  }

  pub fn with_scroll(mut self, v: usize) -> Self {
    self.scroll = v;
    self
  }
  /// The keybindings section to be displayed
  pub fn with_keys<K: AsRef<str>, V: AsRef<str>>(mut self, keys: &[(K, V)]) -> Self {
    self.keys.extend(
      keys
        .iter()
        .map(|(k, v)| [k.as_ref().to_string(), v.as_ref().to_string()])
        .collect::<Vec<_>>(),
    );
    self
  }

  pub fn handle_key(
    key: KeyEvent,
    show: &mut bool,
    num_keys: usize,
    scroll: &mut usize,
    scroll_state: &mut ScrollbarState,
  ) -> bool {
    if key.code == KeyCode::Char('h') {
      *show = !*show;
      if *show {
        *scroll = 0;
        *scroll_state = scroll_state.position(*scroll);
      }
      return true;
    }
    if !*show {
      return false;
    }
    if key.code == KeyCode::Esc || key.code == KeyCode::Char('q') {
      *show = !*show;
      if *show {
        *scroll = 0;
        *scroll_state = scroll_state.position(*scroll);
      }
    } else if key.code == KeyCode::Down {
      if *scroll < num_keys.saturating_sub(1) {
        *scroll += 1;
        *scroll_state = scroll_state.position(*scroll);
        crate::dbg!("Scroll help: {}", *scroll);
      }
    } else if key.code == KeyCode::Up {
      *scroll = scroll.saturating_sub(1);
      *scroll_state = scroll_state.position(*scroll);
      crate::dbg!("Scroll help: {}", *scroll);
    }
    return true;
  }
}

impl StatefulWidget for HelpMenu {
  type State = ScrollbarState;

  fn render(
    self,
    area: ratatui::prelude::Rect,
    buf: &mut ratatui::prelude::Buffer,
    state: &mut ScrollbarState,
  ) where
    Self: Sized,
  {
    let area = popup_area(area, 40, 80);
    Clear::default().render(area, buf);
    let mut help_col_widths = vec![0; 2];
    let mut final_help = vec![];
    for hl in &self.keys {
      help_col_widths[0] = help_col_widths[0].max(hl[0].len());
      help_col_widths[1] = help_col_widths[1].max(hl[1].len());
    }
    for hl in &self.keys {
      let key = format!("{:width$}", hl[0], width = help_col_widths[0]);
      let desc = format!("{:width$}", hl[1], width = help_col_widths[1]);
      final_help.push(Line::default().spans([key, " ".into(), desc]));
    }

    *state = state.content_length(self.keys.len());

    Paragraph::new(final_help)
      .block(
        Block::bordered()
          .title("Help menu")
          .title_alignment(Alignment::Center),
      )
      .centered()
      .scroll((self.scroll as u16, 0))
      .on_black()
      .render(area, buf);
    if self.keys.len() + 2 >= area.height as usize {
      Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"))
        .render(area, buf, state)
    }
  }
}

fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
  let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
  let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
  let [area] = vertical.areas(area);
  let [area] = horizontal.areas(area);
  area
}
