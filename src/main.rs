use cargo_nbuild::Debug;
use core::panic;

use std::{
  collections::VecDeque,
  error::Error,
  fs::File,
  io::{self, BufRead, BufReader, Read},
  ops::{Deref, DerefMut},
  os,
  process::{Child, Command, Stdio},
  sync::{
    mpsc::{self, channel, Receiver, RecvTimeoutError, Sender},
    Arc, Mutex, MutexGuard, PoisonError,
  },
  thread::{spawn, JoinHandle},
  time::{Duration, Instant},
};

use cargo_nbuild::{CargoBuild, Event, EventBus, TryLockFor};
use lazy_static::lazy_static;
use log::{error, info};
use ratatui::{
  crossterm::event::{self, KeyCode, KeyEventKind},
  layout::{Constraint, Layout},
  prelude::Backend,
  restore,
  style::Stylize,
  widgets::{Block, Paragraph},
  DefaultTerminal, Terminal,
};
use std::io::Write as _;

pub struct App {
  events: EventBus,
  threads: VecDeque<JoinHandle<()>>,
}

impl App {
  pub fn new() -> Self {
    let events = EventBus::new();
    Self {
      events,
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
    let render_events = self.events.clone();
    let monitor_events = self.events.clone();
    let build_events = self.events.clone();
    self.threads = VecDeque::from([
      // render
      spawn(move || render(render_events)),
      // monitor
      spawn(move || monitor_build(monitor_events)),
      // build
      spawn(move || build(build_events)),
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

fn build(events: EventBus) {
  let args = std::env::args().skip(1).collect::<Vec<_>>();
  Debug::log("build thread started");
  match CargoBuild::spawn(args) {
    Ok(mut build) => {
      Debug::log("spawned cargo process");
      let out_buf = BufReader::new(build.stdout.take().unwrap());
      let err_buf = BufReader::new(build.stderr.take().unwrap());

      let stderr_events = events.clone();
      let stdout_events = events.clone();
      let stdout_thread = spawn(move || {
        for line in out_buf.lines() {
          let line = line.expect("invalid output line");
          Debug::log(format!("[stdout] {}", line));
          let _ = stdout_events.send(Event::OutputLine(line));
        }
      });
      let stderr_thread = spawn(move || {
        for line in err_buf.lines() {
          let line = line.expect("invalid error line");
          Debug::log(format!("[stderr] {}", line));
          let _ = stderr_events.send(Event::ErrorLine(line));
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
      // Debug::log(format!("Exit status: {}", exit_status));
      let _ = events.send(Event::FinishedExecution(exit_status));
    }
    Err(e) => Debug::log(format!("error: failed to spawn cargo build, {}", e)),
  }
  Debug::log("build thread stopped");
}

fn monitor_build(events: EventBus) {
  Debug::log("monitor thread started");
  loop {
    match events.recv_timeout(Duration::from_millis(5)) {
      Ok(event) => match event {
        Event::UserQuitRequest => {
          Debug::log("received user quit");
          return;
        }
        Event::FinishedExecution(exit) => {
          Debug::log("build process finished");
          break;
        }
        _ => {}
      },
      Err(e) => {
        if let RecvTimeoutError::Disconnected = e {
          Debug::log(format!("monitor failed to receive events, {}", e));
          break;
        }
      }
    }
  }
  Debug::log("monitor thread stopped");
}

fn render(events: EventBus) {
  Debug::log("render thread started");
  let mut terminal = ratatui::init();
  let _ = terminal.clear();
  App::set_panic_hook();
  let app_result = render_loop(terminal);
  ratatui::restore();
  if let Err(e) = app_result {
    Debug::log(format!("failed to run app, {}", e));
  }
  if let Err(e) = events.send(Event::UserQuitRequest) {
    Debug::log(format!("failed to quit app, {}", e));
  }
  Debug::log("render thread stopped");
}

fn render_loop(mut terminal: DefaultTerminal) -> io::Result<()> {
  loop {
    terminal.draw(|frame| {
      let mut args = vec!["cargo".to_string(), "build".to_string()];
      args.extend(std::env::args().skip(1).collect::<Vec<_>>());
      let [command_area, log_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(frame.area());
      let command = Paragraph::new(format!("cmd: {}", args.join(" "))).block(Block::bordered());
      let log = Paragraph::new("Logs").block(Block::bordered());
      frame.render_widget(command, command_area);
      frame.render_widget(log, log_area);
    })?;

    if let event::Event::Key(key) = event::read()? {
      if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
        break;
      }
    }
  }
  Ok(())
}
