use ratatui::{
    style::Stylize,
    text::Line,
    widgets::{Paragraph, Widget},
};

use crate::BuildEvent;

#[derive(Default)]
pub struct StatusBar {
    event: Option<BuildEvent>,
    num_errors: usize,
    num_warnings: usize,
    num_output_lines: usize,
    num_prepared_lines: usize,
}

impl StatusBar {
    pub fn with_event(mut self, evt: BuildEvent) -> Self {
        self.event = Some(evt);
        self
    }

    pub fn with_num_errors(mut self, n: usize) -> Self {
        self.num_errors = n;
        self
    }

    pub fn with_num_warnings(mut self, n: usize) -> Self {
        self.num_warnings = n;
        self
    }

    pub fn with_num_output_lines(mut self, n: usize) -> Self {
        self.num_output_lines = n;
        self
    }

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
