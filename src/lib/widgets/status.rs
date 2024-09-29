use ratatui::{
  style::Stylize,
  text::Line,
  widgets::{Paragraph, Widget},
};

use crate::BuildEvent;

/// The status bar widget displays various informations about
/// the current build status.
#[derive(Default)]
pub struct StatusBar {
  event: Option<BuildEvent>,
  num_errors: usize,
  num_warnings: usize,
  num_output_lines: usize,
  num_prepared_lines: usize,
}

impl StatusBar {
  /// Set the build event to be displayed
  pub fn with_event(mut self, evt: BuildEvent) -> Self {
    self.event = Some(evt);
    self
  }

  /// Set the number of errors
  pub fn with_num_errors(mut self, n: usize) -> Self {
    self.num_errors = n;
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
}

impl Widget for StatusBar {
  fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
  where
    Self: Sized,
  {
    if let Some(evt) = self.event.as_ref() {
      let status_bar = Paragraph::new(match evt {
        BuildEvent::BuildStarted => {
          Line::default().spans(["Build ".into(), "running".gray(), "⌛".into()])
        }
        BuildEvent::BuildFinished(exit) => Line::default().spans([
          "Build ".into(),
          "finished".bold(),
          match exit.success() {
            true => " ✓".bold().green(),
            false => " ✗".bold().red(),
          },
          " | ".into(),
          match exit.success() {
            true => format!("{}", exit).dim(),
            false => format!("{}", exit).into(),
          },
          " | ".into(),
          match self.num_errors {
            0 => format!("{} error(s)", self.num_errors).dim(),
            _ => format!("{} error(s)", self.num_errors).red(),
          },
          " | ".into(),
          match self.num_warnings {
            0 => format!("{} warning(s)", self.num_warnings).dim(),
            _ => format!("{} warning(s)", self.num_warnings).yellow(),
          },
          " | ".into(),
          match self.num_prepared_lines == self.num_output_lines {
            true => format!("{} line(s)", self.num_output_lines).dim(),
            false => format!(
              "{}/{} line(s) prepared",
              self.num_prepared_lines, self.num_output_lines
            )
            .dim(),
          },
        ]),
      });
      status_bar.render(area, buf);
    }
  }
}
