use crate::{
  BuildEntry, BuildEvent, Debug, Origin,
};

use std::{
  collections::VecDeque,
  io::{stdout},
  sync::mpsc::{channel},
  thread::{spawn, JoinHandle},
};

use ratatui::{
  crossterm::{
    event::{
      DisableMouseCapture, EnableMouseCapture,
    },
    execute,
  },
  restore,
};

use super::{AppOptions, Builder, Renderer, Scanner};

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

    let th_tx_events = tx_build_events.clone();
    self.threads = VecDeque::from([
      // render
      spawn(move || {
        Renderer::new(
          render_options,
          terminal,
          tx_user_quit,
          rx_build_output,
          th_tx_events,
          rx_build_events,
        )
        .run()
      }),
      // build
      spawn(move || match build_options.stdin {
        true => Scanner::new(Origin::Stdin, tx_build_output, tx_build_events).run(),
        false => Builder::new(build_options, tx_build_output, tx_build_events).run(),
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
