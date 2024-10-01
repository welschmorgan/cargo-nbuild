use crate::{
  dbg, BuildEntry, BuildEvent, BuildOutput, Debug, HelpMenu, LogView, Markers, Origin, StatusBar,
};

use std::{
  collections::VecDeque,
  io::{self, stdin, stdout, BufRead, BufReader},
  process::ExitStatus,
  sync::mpsc::{channel, Receiver, Sender},
  thread::{spawn, JoinHandle},
  time::Duration,
};

use crate::BuildCommand;
use ratatui::{
  crossterm::{
    event::{
      self, DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyEventKind,
      MouseEventKind,
    },
    execute,
  },
  layout::{Constraint, Layout, Rect},
  restore,
  style::Stylize,
  text::Line,
  widgets::{Block, Paragraph, ScrollbarState},
  DefaultTerminal,
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
];

/// Represent the application data
pub struct App {
  options: AppOptions,
  threads: VecDeque<JoinHandle<()>>,
}

impl App {
  /// Construct a new App instance
  pub fn new(options: AppOptions) -> Self {
    Self {
      options,
      threads: VecDeque::new(),
    }
  }

  /// Define the panic hook to restore the terminal to it's default state after panics
  fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
      let _ = restore();
      let _ = execute!(stdout(), DisableMouseCapture);
      Debug::log(format!("Panic {:?}", panic_info));
      hook(panic_info);
    }));
  }

  /// Run the whole application
  pub fn run(&mut self) -> Result<(), crate::Error> {
    let mut terminal = ratatui::init();
    let _ = terminal.clear();
    let _ = execute!(stdout(), EnableMouseCapture);
    App::set_panic_hook();

    let (tx_user_quit, _rx_user_quit) = channel::<bool>();
    let (tx_build_output, rx_build_output) = channel::<Vec<BuildEntry>>();
    let (tx_build_events, rx_build_events) = channel::<BuildEvent>();
    let render_options = self.options.clone();
    let build_options = self.options.clone();

    self.threads = VecDeque::from([
      // render
      spawn(move || {
        render(
          render_options,
          terminal,
          tx_user_quit,
          rx_build_output,
          rx_build_events,
        )
      }),
      // build
      spawn(move || match build_options.stdin {
        true => scanner(Origin::Stdin, tx_build_output, tx_build_events),
        false => build(build_options, tx_build_output, tx_build_events),
      }),
    ]);
    let mut th_id = 0;
    while let Some(th) = self.threads.pop_front() {
      Debug::log(format!("Waiting for thread {}", th_id));
      if let Err(e) = th.join() {
        Debug::log(format!("failed to join thread #{}, {:?}", th_id, e))
      }
      th_id += 1
    }
    Debug::log(format!("Done with this shit..."));
    Ok(())
  }
}

const THREADED_SCANNER: bool = false;

/// The stdin scanner thread
fn scanner(
  origin: Origin,
  build_output: Sender<Vec<BuildEntry>>,
  build_events: Sender<BuildEvent>,
) {
  crate::dbg!("scan thread started on {:?}", origin);
  let _ = build_events.send(BuildEvent::BuildStarted);
  Debug::log("spawned cargo process");
  let buf = BufReader::new(stdin());
  let events = build_output.clone();
  let f = move || {
    for line in buf.lines() {
      let line = line.expect("invalid input line").replace("\x00", "");
      crate::dbg!("[stdin] {}", line);
      let _ = events.send(vec![BuildEntry::new(line, origin)]);
    }
  };
  if THREADED_SCANNER {
    let thread = spawn(f);
    Debug::log("Waiting for scanner thread");
    thread
      .join()
      .expect("failed to join process scanner thread");
  } else {
    f();
  }
  let exit_status = ExitStatus::default();
  let _ = build_events.send(BuildEvent::BuildFinished(exit_status));
  Debug::log(format!("Exit status: {}", exit_status));
  Debug::log("scan thread stopped");
}

/// The `cargo build` thread. It will run the [`BuildCommand`]
/// and push output lines to [`BuildOutput`]
fn build(
  options: AppOptions,
  build_output: Sender<Vec<BuildEntry>>,
  build_events: Sender<BuildEvent>,
) {
  let args = options.build_args;
  Debug::log("build thread started");
  match BuildCommand::spawn(args) {
    Ok(mut build) => {
      let _ = build_events.send(BuildEvent::BuildStarted);
      Debug::log("spawned cargo process");
      let out_buf = BufReader::new(build.stdout.take().unwrap());
      let err_buf = BufReader::new(build.stderr.take().unwrap());

      let stderr_events = build_output.clone();
      let stdout_events = build_output.clone();
      let stdout_thread = spawn(move || {
        for line in out_buf.lines() {
          let line = line.expect("invalid output line");
          Debug::log(format!("[stdout] {}", line));
          let _ = stdout_events.send(vec![BuildEntry::new(line, Origin::Stdout)]);
        }
      });
      let stderr_thread = spawn(move || {
        for line in err_buf.lines() {
          let line = line.expect("invalid error line");
          Debug::log(format!("[stderr] {}", line));
          let _ = stderr_events.send(vec![BuildEntry::new(line, Origin::Stderr)]);
        }
      });
      // Debug::log("Waiting for stdout/err threads");
      stdout_thread
        .join()
        .expect("failed to join process reader thread");
      stderr_thread
        .join()
        .expect("failed to join process reader thread");
      // Debug::log("Done waiting for stdout/err threads");

      let exit_status = build.wait().expect("failed to wait for cargo");

      let _ = build_events.send(BuildEvent::BuildFinished(exit_status));
      Debug::log(format!("Exit status: {}", exit_status));
    }
    Err(e) => Debug::log(format!("error: failed to spawn cargo build, {}", e)),
  }
  Debug::log("build thread stopped");
}

/// The rendering thread, draws the terminal UI
fn render(
  options: AppOptions,
  terminal: DefaultTerminal,
  user_quit: Sender<bool>,
  build_output: Receiver<Vec<BuildEntry>>,
  build_events: Receiver<BuildEvent>,
) {
  Debug::log("render thread started");
  let app_result = render_loop(options, terminal, user_quit, build_output, build_events);
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
  loop {
    build.pull(&build_output);
    // let output_changed = build.prepare();
    build.prepare();
    *markers.tags_mut() = build.markers().tags().clone();
    if let Some(selected) = markers.selected() {
      build.markers_mut().select(selected);
    }
    let build_lines = build.display();
    if let Ok(e) = build_events.try_recv() {
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
        let status_bar = StatusBar::default()
          .with_event(*status)
          .with_num_prepared_lines(build.cursor())
          .with_num_output_lines(build.entries().len())
          .with_num_notes(num_notes)
          .with_num_errors(num_errs)
          .with_num_warnings(num_warns);
        frame.render_widget(status_bar, status_area);
      }
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
            handle_key_press(
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
      scroll_to_element(entry_id, scroll, log_area);
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
      scroll_to_element(entry_id, scroll, log_area);
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
