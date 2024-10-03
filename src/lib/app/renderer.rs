use std::{
  cell::RefCell,
  io::{self, stdout},
  rc::Rc,
  sync::mpsc::{channel, Receiver, Sender},
  time::Duration,
};

use ratatui::{
  crossterm::{
    event::{self, DisableMouseCapture, KeyCode, KeyEvent, KeyEventKind, MouseEventKind},
    execute,
  },
  layout::{Constraint, Layout, Rect},
  style::{Style, Stylize},
  text::Line,
  widgets::{Block, Paragraph, ScrollbarState},
  DefaultTerminal,
};

use crate::{
  BuildEntry, BuildEvent, BuildOutput, BuildTagKind, Debug, HelpMenu, LogView, MarkedBlock,
  MarkerSelection, Markers, SearchBar, SearchState, StatusBar, StatusMessage,
};

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
  ("/", "enter search mode"),
  ("Esc", "exit search mode"),
  ("e", "show first error"),
  ("w", "show first warning"),
  ("n", "show first note"),
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
    Self::restore_terminal();
    if let Err(e) = app_result {
      Debug::log(format!("failed to run app, {}", e));
    }
    Debug::log("render thread stopped");
  }

  pub fn restore_terminal() {
    ratatui::restore();
    let _ = execute!(stdout(), DisableMouseCapture);
  }

  fn set_cursor_visible(terminal: &mut DefaultTerminal, v: bool) {
    if v {
      if let Err(e) = terminal.show_cursor() {
        crate::dbg!("failed to show cursor: {}", e);
      }
    } else {
      if let Err(e) = terminal.set_cursor_position((0, 0)) {
        crate::dbg!("failed to set cursor position: {}", e);
      }
      if let Err(e) = terminal.hide_cursor() {
        crate::dbg!("failed to hide cursor: {}", e);
      }
    }
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
    let mut build = BuildOutput::default()
      .with_noise_removed(false)
      .with_build_events(tx_build_events.clone());
    let mut vertical_scroll_state = ScrollbarState::default();
    let mut vertical_scroll: usize = 0;
    let [mut command_area, mut log_area] = [Rect::default(), Rect::default()];
    let mut main_pane = Rect::default();
    let mut shortcuts_area = Rect::default();
    let mut top_area = Rect::default();
    let mut bottom_area = Rect::default();
    let mut search_area = Rect::default();
    let mut status_area = Rect::default();
    let mut status_entry: Option<StatusMessage> = None;
    let mut build_status_entry: Option<BuildEvent> = None;
    let mut show_help = false;
    let mut markers = Markers::default();
    let _frame_area: Rect = terminal.get_frame().area();
    let status_bar = Rc::new(RefCell::new(StatusBar::default()));
    let mut search_state: Option<SearchState> = None;
    let (tx_search_query, rx_search_query) = channel::<String>();
    let mut _last_search_result: Option<(MarkedBlock<'_>, MarkerSelection)> = None;
    let mut stop = false;
    while !stop {
      build.pull(&build_output);
      if build.prepare() {
        markers.set_selection(build.markers_mut().selection().cloned());
      }
      *markers.tags_mut() = build.markers().tags().clone();
      let mut search_selection = None;
      if let Ok(query) = rx_search_query.try_recv() {
        crate::dbg!("Searching for '{}'", query);
        search_selection = if let Some((block, selection)) = build.search(&query) {
          crate::dbg!(
            "Found in block #{} -> {:?}\n{}",
            block.marker_id(),
            selection,
            block
              .lines()
              .iter()
              .map(|line| format!("  | {}", line))
              .collect::<Vec<_>>()
              .join("\n")
          );
          _last_search_result = Some((block.clone(), selection.clone()));
          search_state = None;
          status_entry = Some(StatusMessage::new([(
            format!("Show search result {}/{}", block.marker_id(), markers.len()),
            Style::default(),
          )]));
          markers.set_selection(Some(selection));
          markers.selection()
        } else {
          status_entry = Some(StatusMessage::new([
            (" ✗ ".to_string(), Style::default().bold().red()),
            (format!("'{}' not found", query), Style::default()),
          ]));
          None
        };
      }
      build
        .markers_mut()
        .set_selection(markers.selection().cloned());
      if let Some(search_sel) = search_selection {
        build.select_entry(search_sel.entry_id, search_sel.region.clone());
      }
      let build_lines = build.display();
      if let Ok(e) = build_events.try_recv() {
        crate::dbg!("Received {:?}", e);
        build_status_entry = Some(e);
      }
      // if first_render || output_changed || key_event {
      let (num_errs, num_warns, num_notes) = (
        build.errors().len(),
        build.warnings().len(),
        build.notes().len(),
      );
      Self::set_cursor_visible(&mut terminal, search_state.is_some());
      terminal.draw(|frame| {
        [top_area, main_pane] =
          Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(frame.area());
        [command_area, shortcuts_area] =
          Layout::horizontal([Constraint::Percentage(50), Constraint::Fill(1)]).areas(top_area);
        [log_area, bottom_area] =
          Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(main_pane);
        [search_area, status_area] = match search_state {
          Some(_) => {
            Layout::horizontal([Constraint::Percentage(50), Constraint::Fill(1)]).areas(bottom_area)
          }
          None => {
            Layout::horizontal([Constraint::Length(0), Constraint::Fill(1)]).areas(bottom_area)
          }
        };

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

        if status_entry.is_some() || build_status_entry.is_some() {
          let mut new_status = *status_bar.borrow();
          match (status_entry.as_ref(), build_status_entry.as_ref()) {
            (Some(status_msg), Some(build_event)) => {
              new_status = new_status
                .with_event(*build_event)
                .with_message(*status_msg);
            }
            (Some(status_msg), None) => {
              new_status = new_status.with_message(*status_msg);
            }
            (None, Some(build_event)) => {
              new_status = new_status.with_event(*build_event);
            }
            (None, None) => {}
          }
          *status_bar.borrow_mut() = new_status
            .with_num_prepared_lines(build.cursor())
            .with_num_output_lines(build.entries().len())
            .with_num_notes(num_notes)
            .with_num_errors(num_errs)
            .with_num_warnings(num_warns);
          status_entry = None;
          build_status_entry = None;
        }
        frame.render_widget(*status_bar.borrow(), status_area);
        let log_view = LogView::default()
          .with_content(build_lines.clone())
          .with_scroll(vertical_scroll);
        frame.render_stateful_widget(log_view, log_area, &mut vertical_scroll_state);
        // frame.render_stateful_widget(log_view, log_area, &mut list_state);
        frame.render_widget(shortcuts, shortcuts_area);
        frame.render_widget(command, command_area);
        if search_state.is_some() {
          frame.render_stateful_widget(SearchBar, search_area, &mut search_state);
          let mut cursor_pos = (search_area.x, search_area.y);
          if let Some(state) = search_state.as_ref() {
            cursor_pos.0 += state.cursor_position() as u16;
          }
          frame.set_cursor_position(cursor_pos);
        }
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
              Self::handle_key_press(
                key,
                &mut vertical_scroll,
                &mut vertical_scroll_state,
                &mut markers,
                &mut stop,
                user_quit.clone(),
                &log_area,
                &build,
                &build_lines,
                &mut search_state,
                tx_search_query.clone(),
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

  fn find_first_marker(markers: &Markers, kind: BuildTagKind) -> Option<MarkerSelection> {
    if let Some((marker_id, (entry_id, _tag))) = markers
      .iter()
      .enumerate()
      .find(|(_marker_id, (_entry_id, tag))| *tag == kind)
    {
      return Some(MarkerSelection::new(marker_id, *entry_id, None));
    }
    return None;
  }

  /// Handle user keypresses
  fn handle_key_press(
    key: KeyEvent,
    scroll: &mut usize,
    state: &mut ScrollbarState,
    markers: &mut Markers,
    stop: &mut bool,
    user_quit: Sender<bool>,
    log_area: &Rect,
    build_output: &BuildOutput,
    build_lines: &Vec<Line<'_>>,
    search_value: &mut Option<SearchState>,
    search_query: Sender<String>,
    show_help: &mut bool,
  ) {
    if SearchBar::handle_key(key, search_value, search_query) {
      return;
    }
    if key.code == KeyCode::Char('q') {
      if let Err(e) = user_quit.send(true) {
        Debug::log(format!("failed to quit app, {}", e));
      }
      *stop = true;
    } else if key.code == KeyCode::Char('e') {
      if let Some(sel) = Self::find_first_marker(markers, BuildTagKind::Error) {
        Self::select_marker(&sel, markers, scroll, state, log_area);
      }
    } else if key.code == KeyCode::Char('w') {
      if let Some(sel) = Self::find_first_marker(markers, BuildTagKind::Warning) {
        Self::select_marker(&sel, markers, scroll, state, log_area);
      }
    } else if key.code == KeyCode::Char('n') {
      if let Some(sel) = Self::find_first_marker(markers, BuildTagKind::Note) {
        Self::select_marker(&sel, markers, scroll, state, log_area);
      }
    } else if key.code == KeyCode::Char('j') {
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
        let marker_id = markers.select_last().cloned();
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
        let marker_id = markers.select_first().cloned();
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
      if let Some(previous) = markers.previous_selection() {
        Self::select_marker(&previous, markers, scroll, state, log_area);
      }
    } else if key.code == KeyCode::Down {
      if let Some(next) = markers.next_selection() {
        Self::select_marker(&next, markers, scroll, state, log_area);
      }
    }
  }

  fn select_marker(
    selection: &MarkerSelection,
    markers: &mut Markers,
    scroll: &mut usize,
    state: &mut ScrollbarState,
    log_area: &Rect,
  ) {
    if markers.is_empty() {
      *scroll = selection.entry_id;
      *state = state.position(*scroll);
    } else {
      markers.select(selection.marker_id, selection.region.clone());
      let entry_id = markers.selected_entry().unwrap_or_default();
      if entry_id >= *scroll + (log_area.height as usize) {
        *scroll = entry_id;
        *state = state.position(*scroll);
      } else if entry_id < *scroll {
        *scroll = scroll.saturating_sub(log_area.height as usize);
        *state = state.position(*scroll);
      }
    }
  }
}
