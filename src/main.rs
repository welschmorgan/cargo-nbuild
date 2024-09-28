use std::{
  collections::VecDeque,
  io,
  sync::{
    mpsc::{self, channel, Receiver, Sender},
    Arc, Mutex,
  },
  thread::spawn,
  time::Duration,
};

use cargo_nbuild::{Event, EventBus};
use log::{error, info};
use ratatui::{
  crossterm::event::{self, KeyCode, KeyEventKind},
  layout::{Constraint, Layout},
  prelude::Backend,
  style::Stylize,
  widgets::{Block, Paragraph},
  DefaultTerminal, Terminal,
};

pub struct Logs {}

fn main() -> io::Result<()> {
  pretty_env_logger::init();
  let mut terminal = ratatui::init();
  terminal.clear()?;
  let events = EventBus::new();
  let render_events = events.clone();
  let monitor_events = events.clone();
  let mut threads = VecDeque::from([
    // render
    spawn(move || render(render_events, terminal)),
    // build
    spawn(move || monitor_build(monitor_events)),
  ]);
  let mut th_id = 0;
  while let Some(th) = threads.pop_front() {
    if let Err(e) = th.join() {
      error!("failed to join thread #{}, {:?}", th_id, e)
    }
    th_id += 1
  }
  return Ok(());
}

fn monitor_build(events: EventBus) {
  loop {
    if let Ok(evt) = events.recv_timeout(Duration::from_millis(5)) {
      match evt {
        Event::Dummy => {}
        Event::Quit => break,
      }
    }
    std::thread::sleep(Duration::from_secs_f32(0.05));
  }
}

fn render(events: EventBus, term: DefaultTerminal) {
  let app_result = run(term);
  ratatui::restore();
  if let Err(e) = app_result {
    error!("failed to run app, {}", e)
  }
  if let Err(e) = events.send(Event::Quit) {
    error!("failed to quit app, {}", e)
  }
}

fn run(mut terminal: DefaultTerminal) -> io::Result<()> {
  loop {
    terminal.draw(|frame| {
      let args = std::env::args().collect::<Vec<_>>();
      let [command_area, log_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(frame.area());
      let command = Paragraph::new(format!("cmd: {}", args.join(" "))).block(Block::bordered());
      let log = Paragraph::new("Logs").block(Block::bordered());
      frame.render_widget(command, command_area);
      frame.render_widget(log, log_area);
    })?;

    if let event::Event::Key(key) = event::read()? {
      if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
        return Ok(());
      }
    }
  }
}
