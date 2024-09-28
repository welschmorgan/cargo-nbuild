use std::collections::HashMap;

use ratatui::{
  layout::{Constraint, Flex, Layout, Rect},
  style::Stylize,
  text::Line,
  widgets::{Block, Clear, Paragraph, Widget},
};

pub struct HelpMenu {
  keys: Vec<[String; 2]>,
}

impl HelpMenu {
  pub fn new() -> Self {
    Self { keys: vec![] }
  }

  pub fn with_keys<K: AsRef<str>, V: AsRef<str>>(mut self, keys: &[(K, V)]) -> Self {
    self.keys.extend(
      keys
        .iter()
        .map(|(k, v)| [k.as_ref().to_string(), v.as_ref().to_string()])
        .collect::<Vec<_>>(),
    );
    self
  }
}

impl Widget for HelpMenu {
  fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
  where
    Self: Sized,
  {
    let area = popup_area(area, 60, 40);
    Clear::default().render(area, buf);
    let mut help_col_widths = vec![0; 2];
    let mut final_help = vec![];
    for hl in &self.keys {
      help_col_widths[0] = help_col_widths[0].max(hl[0].len());
      help_col_widths[1] = help_col_widths[1].max(hl[1].len());
    }
    for hl in self.keys {
      let key = format!("{:width$}", hl[0], width = help_col_widths[0]);
      let desc = format!("{:width$}", hl[1], width = help_col_widths[1]);
      final_help.push(Line::default().spans([key, " ".into(), desc]));
    }
    Paragraph::new(final_help)
      .block(Block::bordered())
      .centered()
      .on_black()
      .render(area, buf);
  }
}

fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
  let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
  let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
  let [area] = vertical.areas(area);
  let [area] = horizontal.areas(area);
  area
}
