use cargo_nbuild::{BuildEntry, Channel, Debug, Origin};
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

use cargo_nbuild::{CargoBuild, TryLockFor};
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

pub type UserQuitChannel = Channel<bool>;
pub type BuildOutputChannel = Channel<BuildEntry>;

pub struct App {
  build_output: Arc<BuildOutputChannel>,
  user_quit: Arc<UserQuitChannel>,
  threads: VecDeque<JoinHandle<()>>,
}

impl App {
  pub fn new() -> Self {
    Self {
      build_output: Arc::new(Channel::new()),
      user_quit: Arc::new(Channel::new()),
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
    let user_quit = self.user_quit.clone();
    let build_output = self.build_output.clone();
    self.threads = VecDeque::from([
      // render
      spawn(move || render(user_quit)),
      // build
      spawn(move || build(build_output)),
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

fn build(build_output: Arc<BuildOutputChannel>) {
  let args = std::env::args().skip(1).collect::<Vec<_>>();
  Debug::log("build thread started");
  match CargoBuild::spawn(args) {
    Ok(mut build) => {
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
      Debug::log(format!("Exit status: {}", exit_status));
    }
    Err(e) => Debug::log(format!("error: failed to spawn cargo build, {}", e)),
  }
  Debug::log("build thread stopped");
}

fn render(user_quit: Arc<UserQuitChannel>) {
  Debug::log("render thread started");
  let mut terminal = ratatui::init();
  let _ = terminal.clear();
  App::set_panic_hook();
  let app_result = render_loop(terminal, user_quit);
  ratatui::restore();
  if let Err(e) = app_result {
    Debug::log(format!("failed to run app, {}", e));
  }
  Debug::log("render thread stopped");
}

fn render_loop(mut terminal: DefaultTerminal, user_quit: Arc<UserQuitChannel>) -> io::Result<()> {
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
        if let Err(e) = user_quit.send(true) {
          Debug::log(format!("failed to quit app, {}", e));
        }
        break;
      }
    }
  }
  Ok(())
}
