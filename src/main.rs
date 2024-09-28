use cargo_nbuild::{BuildEntry, BuildEvent, BuildOutput, Debug, Origin};
use core::panic;

use std::{
  cell::RefCell,
  collections::VecDeque,
  error::Error,
  fs::File,
  io::{self, BufRead, BufReader, Read},
  ops::{Deref, DerefMut},
  os,
  process::{Child, Command, Stdio},
  rc::Rc,
  sync::{
    mpsc::{self, channel, Receiver, RecvTimeoutError, Sender},
    Arc, Mutex, MutexGuard, PoisonError,
  },
  thread::{spawn, JoinHandle},
  time::{Duration, Instant},
};

use cargo_nbuild::{BuildCommand, TryLockFor};
use lazy_static::lazy_static;
use log::{error, info};
use ratatui::{
  crossterm::event::{self, KeyCode, KeyEventKind},
  layout::{Constraint, Layout, Rect},
  prelude::Backend,
  restore,
  style::Stylize,
  text::{Line, Span},
  widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
  DefaultTerminal, Terminal,
};
use std::io::Write as _;

pub struct App {
  threads: VecDeque<JoinHandle<()>>,
}

impl App {
  pub fn new() -> Self {
    Self {
      threads: VecDeque::new(),
    }
  }

  fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
      let _ = restore();
      Debug::log("Recovered from panic!");
      hook(panic_info);
    }));
  }

  pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
    let (tx_user_quit, rx_user_quit) = channel::<bool>();
    let (tx_build_output, rx_build_output) = channel::<BuildEntry>();
    let (tx_build_events, rx_build_events) = channel::<BuildEvent>();
    self.threads = VecDeque::from([
      // render
      spawn(move || render(tx_user_quit, rx_build_output, rx_build_events)),
      // build
      spawn(move || build(tx_build_output, tx_build_events)),
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

fn main() -> Result<(), Box<dyn Error>> {
  App::new().run()
}

fn build(build_output: Sender<BuildEntry>, build_events: Sender<BuildEvent>) {
  let args = std::env::args().skip(1).collect::<Vec<_>>();
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
          let _ = stdout_events.send(BuildEntry::new(line, Origin::Stdout));
        }
      });
      let stderr_thread = spawn(move || {
        for line in err_buf.lines() {
          let line = line.expect("invalid error line");
          Debug::log(format!("[stderr] {}", line));
          let _ = stderr_events.send(BuildEntry::new(line, Origin::Stderr));
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

fn render(
  user_quit: Sender<bool>,
  build_output: Receiver<BuildEntry>,
  build_events: Receiver<BuildEvent>,
) {
  Debug::log("render thread started");
  let mut terminal = ratatui::init();
  let _ = terminal.clear();
  App::set_panic_hook();
  let app_result = render_loop(terminal, user_quit, build_output, build_events);
  ratatui::restore();
  if let Err(e) = app_result {
    Debug::log(format!("failed to run app, {}", e));
  }
  Debug::log("render thread stopped");
}

fn render_loop(
  mut terminal: DefaultTerminal,
  user_quit: Sender<bool>,
  build_output: Receiver<BuildEntry>,
  build_events: Receiver<BuildEvent>,
) -> io::Result<()> {
  let mut build = BuildOutput::default();
  let mut vertical_scroll_state = ScrollbarState::default();
  let mut vertical_scroll: usize = 0;
  let [mut command_area, mut log_area] = [Rect::default(), Rect::default()];
  let mut main_pane = Rect::default();
  let mut status_area = Rect::default();
  let mut status_entry: Option<BuildEvent> = None;
  loop {
    build.pull(&build_output);
    build.prepare();
    let build_lines = build.display();
    if let Ok(e) = build_events.try_recv() {
      status_entry = Some(e);
    }
    let (num_errs, num_warns) = (build.errors().len(), build.warnings().len());
    terminal.draw(|frame| {
      [command_area, main_pane] =
        Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(frame.area());
      [log_area, status_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(main_pane);

      let mut args = vec![
        "cmd".bold(),
        ":".into(),
        " ".into(),
        "cargo".dim(),
        " ".into(),
        "build".dim(),
      ];
      args.extend(
        std::env::args()
          .skip(1)
          .map(|arg| arg.into())
          .collect::<Vec<_>>(),
      );
      let command = Paragraph::new(Line::default().spans(args)).block(Block::bordered());

      if let Some(status) = status_entry.as_ref() {
        let status_bar = Paragraph::new(match status {
          BuildEvent::BuildStarted => Line::default().spans(["Build ".into(), "running".gray()]),
          BuildEvent::BuildFinished(exit) => Line::default().spans([
            "Build ".into(),
            "finished".bold(),
            " | ".into(),
            match exit.success() {
              true => format!("{}", exit).dim(),
              false => format!("{}", exit).into(),
            },
            " | ".into(),
            match num_errs {
              0 => format!("{} error(s)", num_errs).dim(),
              _ => format!("{} error(s)", num_errs).red(),
            },
            " | ".into(),
            match num_warns {
              0 => format!("{} warning(s)", num_warns).dim(),
              _ => format!("{} warning(s)", num_warns).yellow(),
            },
          ]),
        });
        frame.render_widget(status_bar, status_area);
      }
      vertical_scroll_state = vertical_scroll_state.content_length(build_lines.len());
      let log = Paragraph::new(build_lines.clone())
        .gray()
        .block(Block::bordered().gray())
        .scroll((vertical_scroll as u16, 0));
      // let log = Paragraph::new(lines).block(Block::bordered());
      frame.render_widget(command, command_area);
      frame.render_widget(log, log_area);
      frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
          .begin_symbol(Some("↑"))
          .end_symbol(Some("↓")),
        log_area,
        &mut vertical_scroll_state,
      );
    })?;

    if event::poll(Duration::from_millis(1))? {
      if let event::Event::Key(key) = event::read()? {
        if key.kind == KeyEventKind::Press {
          if key.code == KeyCode::Char('q') {
            if let Err(e) = user_quit.send(true) {
              Debug::log(format!("failed to quit app, {}", e));
            }
            break;
          } else if key.code == KeyCode::Char('j') {
            if vertical_scroll < build_lines.len().saturating_sub(log_area.height as usize) {
              vertical_scroll = vertical_scroll.saturating_add(1);
              vertical_scroll_state = vertical_scroll_state.position(vertical_scroll);
            }
          } else if key.code == KeyCode::Char('k') {
            vertical_scroll = vertical_scroll.saturating_sub(1);
            vertical_scroll_state = vertical_scroll_state.position(vertical_scroll);
          } else if key.code == KeyCode::End {
            vertical_scroll = build_lines.len();
            vertical_scroll_state = vertical_scroll_state.position(vertical_scroll);
          } else if key.code == KeyCode::Home {
            vertical_scroll = 0;
            vertical_scroll_state = vertical_scroll_state.position(vertical_scroll);
          } else if key.code == KeyCode::PageUp {
            vertical_scroll = vertical_scroll.saturating_sub(log_area.height as usize);
            vertical_scroll_state = vertical_scroll_state.position(vertical_scroll);
          } else if key.code == KeyCode::PageDown {
            if vertical_scroll < build_lines.len().saturating_sub(log_area.height as usize) {
              vertical_scroll = vertical_scroll.saturating_add(log_area.height as usize);
              vertical_scroll_state = vertical_scroll_state.position(vertical_scroll);
            }
          }
        }
      }
    }
  }
  Ok(())
}
