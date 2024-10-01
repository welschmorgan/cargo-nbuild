use std::{
  cell::RefCell,
  io::{self, stdout},
  rc::Rc,
  sync::mpsc::{Receiver, Sender},
  time::Duration,
};

use ratatui::{
  crossterm::{
    event::{self, DisableMouseCapture, KeyCode, KeyEvent, KeyEventKind, MouseEventKind},
    execute,
  },
  layout::{Constraint, Layout, Rect},
  style::Stylize,
  text::Line,
  widgets::{Block, Paragraph, ScrollbarState},
  DefaultTerminal,
};

use crate::{BuildEntry, BuildEvent, BuildOutput, Debug, HelpMenu, LogView, Markers, StatusBar};

use super::AppOptions;

/// The key bindings to be displayed on the help menu
const HELP_MENU: &'static [(&'static str, &'static str)] = &[
  ("k", "previous output row"),
  ("j", "next output row"),
  ("PageUp", "previous output row"),
  ("PageDn", "next output row"),
  ("Home", "go to the first output row"),
  ("End", "go to the last output row"),
  ("Up", "go to the previous marker (error/warning/note)"),
  ("Down", "go to the next marker (error/warning/note)"),
];

pub struct Renderer {
  options: AppOptions,
  terminal: DefaultTerminal,
  user_quit: Sender<bool>,
  build_output: Receiver<Vec<BuildEntry>>,
  tx_build_events: Sender<BuildEvent>,
  build_events: Receiver<BuildEvent>,
}

impl Renderer {
  pub fn new(
    options: AppOptions,
    terminal: DefaultTerminal,
    user_quit: Sender<bool>,
    build_output: Receiver<Vec<BuildEntry>>,
    tx_build_events: Sender<BuildEvent>,
    build_events: Receiver<BuildEvent>,
  ) -> Self {
    Self {
      options,
      terminal,
      user_quit,
      build_output,
      tx_build_events,
      build_events,
    }
  }

  /// The rendering thread, draws the terminal UI
  pub fn run(self) {
    Debug::log("render thread started");
    let app_result = Self::render_loop(
      self.options,
      self.terminal,
      self.user_quit,
      self.build_output,
      self.tx_build_events,
      self.build_events,
    );
    ratatui::restore();
    let _ = execute!(stdout(), DisableMouseCapture);
    if let Err(e) = app_result {
      Debug::log(format!("failed to run app, {}", e));
    }
    Debug::log("render thread stopped");
  }

  /// The rendering loop
  fn render_loop(
    options: AppOptions,
    mut terminal: DefaultTerminal,
    user_quit: Sender<bool>,
    build_output: Receiver<Vec<BuildEntry>>,
    tx_build_events: Sender<BuildEvent>,
    build_events: Receiver<BuildEvent>,
  ) -> io::Result<()> {
    let mut build = BuildOutput::default().with_noise_removed(false);
    let mut vertical_scroll_state = ScrollbarState::default();
    let mut vertical_scroll: usize = 0;
    let [mut command_area, mut log_area] = [Rect::default(), Rect::default()];
    let mut main_pane = Rect::default();
    let mut shortcuts_area = Rect::default();
    let mut top_area = Rect::default();
    let mut status_area = Rect::default();
    let mut status_entry: Option<BuildEvent> = None;
    let mut show_help = false;
    let mut markers = Markers::default();
    let frame_area: Rect = terminal.get_frame().area();
    let status_bar = Rc::new(RefCell::new(StatusBar::default()));
    loop {
      build.pull(&build_output);
      // let output_changed = build.prepare();
      build.prepare(tx_build_events.clone());
      *markers.tags_mut() = build.markers().tags().clone();
      if let Some(selected) = markers.selected() {
        build.markers_mut().select(selected);
      }
      let build_lines = build.display();
      if let Ok(e) = build_events.try_recv() {
        crate::dbg!("Received {:?}", e);
        status_entry = Some(e);
      }
      // if first_render || output_changed || key_event {
      let (num_errs, num_warns, num_notes) = (
        build.errors().len(),
        build.warnings().len(),
        build.notes().len(),
      );
      terminal.draw(|frame| {
        [top_area, main_pane] =
          Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(frame.area());
        [command_area, shortcuts_area] =
          Layout::horizontal([Constraint::Percentage(50), Constraint::Fill(1)]).areas(top_area);
        [log_area, status_area] =
          Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(main_pane);

        let mut args = vec!["cmd".bold(), ":".into(), " ".into()];
        if options.stdin {
          args.extend_from_slice(&["stdin".dim()]);
        } else {
          args.extend_from_slice(&["cargo".dim(), " ".into(), "build".dim()]);
        }
        args.extend(
          options
            .build_args
            .iter()
            .flat_map(|arg| vec![" ".into(), arg.into()].into_iter())
            .collect::<Vec<_>>(),
        );
        let command = Paragraph::new(Line::default().spans(args)).block(Block::bordered());
        let shortcuts =
          Paragraph::new(Line::default().spans(["H: Show help"])).block(Block::bordered());

        if let Some(status) = status_entry.as_ref() {
          let new_status = (*status_bar.borrow())
            .with_event(*status)
            .with_num_prepared_lines(build.cursor())
            .with_num_output_lines(build.entries().len())
            .with_num_notes(num_notes)
            .with_num_errors(num_errs)
            .with_num_warnings(num_warns);
          *status_bar.borrow_mut() = new_status;
        }
        frame.render_widget(*status_bar.borrow(), status_area);
        let log_view = LogView::default()
          .with_content(build_lines.clone())
          .with_scroll(vertical_scroll);
        frame.render_stateful_widget(log_view, log_area, &mut vertical_scroll_state);
        // frame.render_stateful_widget(log_view, log_area, &mut list_state);
        frame.render_widget(shortcuts, shortcuts_area);
        frame.render_widget(command, command_area);
        if show_help {
          let help = HelpMenu::new().with_keys(HELP_MENU);
          frame.render_widget(help, frame.area());
        }
      })?;
      // }

      if event::poll(Duration::from_micros(100))? {
        match event::read()? {
          event::Event::Mouse(mouse) => match mouse.kind {
            MouseEventKind::ScrollDown => {
              vertical_scroll = vertical_scroll.saturating_add(1);
              vertical_scroll_state = vertical_scroll_state.position(vertical_scroll);
            }
            MouseEventKind::ScrollUp => {
              vertical_scroll = vertical_scroll.saturating_sub(1);
              vertical_scroll_state = vertical_scroll_state.position(vertical_scroll);
            }
            _ => {}
          },
          event::Event::Key(key) => {
            if key.kind == KeyEventKind::Press {
              if key.code == KeyCode::Char('q') {
                if let Err(e) = user_quit.send(true) {
                  Debug::log(format!("failed to quit app, {}", e));
                }
                break;
              }
              Self::handle_key_press(
                key,
                frame_area,
                &mut vertical_scroll,
                &mut vertical_scroll_state,
                &mut markers,
                &log_area,
                &build_lines,
                &mut show_help,
              );
            }
          }
          _ => {}
        }
      }
    }
    Ok(())
  }

  fn scroll_to_element(index: usize, scroll: &mut usize, log_area: &Rect) {
    if index < *scroll {
      *scroll = index.saturating_sub(log_area.height as usize);
    } else if index >= *scroll + log_area.height as usize {
      *scroll = index;
    }
  }

  /// Handle user keypresses
  fn handle_key_press(
    key: KeyEvent,
    frame_area: Rect,
    scroll: &mut usize,
    state: &mut ScrollbarState,
    markers: &mut Markers,
    log_area: &Rect,
    build_lines: &Vec<Line<'_>>,
    show_help: &mut bool,
  ) {
    if key.code == KeyCode::Char('j') {
      if *scroll < build_lines.len().saturating_sub(log_area.height as usize) {
        *scroll = scroll.saturating_add(1);
        *state = state.position(*scroll);
      }
    } else if key.code == KeyCode::Char('k') {
      *scroll = scroll.saturating_sub(1);
      *state = state.position(*scroll);
    } else if key.code == KeyCode::Char('h') {
      *show_help = !*show_help;
    } else if key.code == KeyCode::End {
      crate::dbg!("goto end");
      if !markers.is_empty() {
        let marker_id = markers.select_last();
        crate::dbg!(
          "marker is now {:?}: {:?}: {:?}",
          marker_id,
          markers.selected_entry(),
          markers
        );
        let entry_id = markers.selected_entry().unwrap_or_default();
        Self::scroll_to_element(entry_id, scroll, log_area);
      } else {
        *scroll = build_lines.len().saturating_sub(log_area.height as usize);
      }
      crate::dbg!("scroll to line {}", *scroll);
      *state = state.position(*scroll);
    } else if key.code == KeyCode::Home {
      crate::dbg!("goto beginning");
      if !markers.is_empty() {
        let marker_id = markers.select_first();
        crate::dbg!(
          "marker is now {:?}: {:?}",
          marker_id,
          markers.selected_entry()
        );
        let entry_id = markers.selected_entry().unwrap_or_default();
        Self::scroll_to_element(entry_id, scroll, log_area);
      } else {
        *scroll = 0;
      }
      crate::dbg!("scroll to line {}", *scroll);
      *state = state.position(*scroll);
    } else if key.code == KeyCode::PageUp {
      *scroll = scroll.saturating_sub(log_area.height as usize);
      *state = state.position(*scroll);
    } else if key.code == KeyCode::PageDown {
      if *scroll < build_lines.len().saturating_sub(log_area.height as usize) {
        *scroll = scroll.saturating_add(log_area.height as usize);
        *state = state.position(*scroll);
      }
    } else if key.code == KeyCode::Up {
      if markers.is_empty() {
        *scroll = scroll.saturating_sub(1);
        *state = state.position(*scroll);
      } else {
        let _ = markers.select_previous();
        let entry_id = markers.selected_entry().unwrap_or_default();
        if entry_id < (*scroll + (frame_area.height as usize)) + 4
        /* status bar is 1, top bar is 3 */
        {
          *scroll = entry_id;
          *state = state.position(*scroll);
        }
      }
    } else if key.code == KeyCode::Down {
      if markers.is_empty() {
        *scroll = scroll.saturating_add(1);
        *state = state.position(*scroll);
      } else {
        let _ = markers.select_next();
        let entry_id = markers.selected_entry().unwrap_or_default();
        if entry_id >= (*scroll + (frame_area.height as usize)) - 4
        /* status bar is 1, top bar is 3 */
        {
          *scroll = entry_id;
          *state = state.position(*scroll);
        }
      }
    }
  }
}
