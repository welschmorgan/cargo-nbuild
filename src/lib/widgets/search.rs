use std::sync::mpsc::Sender;

use ratatui::{
  crossterm::event::{KeyCode, KeyEvent},
  text::Text,
  widgets::{StatefulWidget, Widget},
};

pub enum Direction {
  Forward,
  Backward,
}

pub struct SearchState {
  prompt: String,
  query: String,
  cursor: usize,
}

impl SearchState {
  pub fn cursor_position(&self) -> usize {
    self.prompt.len() + self.cursor
  }

  pub fn pop(&mut self, dir: Direction) -> Option<char> {
    match dir {
      Direction::Backward => {
        if self.cursor > 0 && !self.query.is_empty() {
          let ret = Some(self.query.remove(self.cursor - 1));
          self.cursor -= 1;
          return ret;
        }
      }
      Direction::Forward => {
        if self.cursor < self.query.len() {
          return Some(self.query.remove(self.cursor));
        }
      }
    }
    None
  }
  pub fn push(&mut self, ch: char) {
    if self.cursor != self.query.len() {
      self.query.insert(self.cursor, ch);
    } else {
      self.query.push(ch);
    }
    self.cursor += 1;
  }
}

pub struct SearchBar;

impl SearchBar {
  pub fn handle_key(
    key: KeyEvent,
    state: &mut Option<SearchState>,
    select: Sender<String>,
  ) -> bool {
    if state.is_some() {
      if key.code == KeyCode::Esc {
        *state = None;
      } else if key.code == KeyCode::Backspace {
        state.as_mut().unwrap().pop(Direction::Backward);
      } else if key.code == KeyCode::Delete {
        state.as_mut().unwrap().pop(Direction::Forward);
      } else if key.code == KeyCode::Left {
        let state = state.as_mut().unwrap();
        state.cursor = state.cursor.saturating_sub(1);
      } else if key.code == KeyCode::Right {
        let state = state.as_mut().unwrap();
        if state.cursor < state.query.len() + state.cursor {
          state.cursor = state.cursor.saturating_add(1);
        }
      } else if key.code == KeyCode::Enter {
        let query = state.as_ref().unwrap().query.clone();
        let _ = select.send(query);
      } else if let KeyCode::Char(ch) = key.code {
        state.as_mut().unwrap().push(ch);
      }
      return true;
    } else {
      if key.code == KeyCode::Char('/') {
        *state = Some(SearchState {
          query: String::new(),
          cursor: 0,
          prompt: String::from("> "),
        });
        return true;
      }
    }
    return false;
  }

  pub fn format(state: &Option<SearchState>) -> Option<String> {
    return state
      .as_ref()
      .map(|state| format!("> {}", state.query.as_str()));
  }
}

impl StatefulWidget for SearchBar {
  type State = Option<SearchState>;

  fn render(
    self,
    area: ratatui::prelude::Rect,
    buf: &mut ratatui::prelude::Buffer,
    state: &mut Self::State,
  ) {
    if let Some(text) = Self::format(state) {
      let text = Text::from(text);
      text.render(area, buf);
    }
  }
}
