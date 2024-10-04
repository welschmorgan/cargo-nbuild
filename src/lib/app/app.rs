use crate::{
  default_system_location, init_rules, load_rules, save_rules, set_active_rule, BuildEntry,
  BuildEvent, Debug, Origin, Rule, DEFAULT_RULES,
};

use std::{
  collections::VecDeque,
  io::stdout,
  path::PathBuf,
  process::exit,
  sync::mpsc::channel,
  thread::{spawn, JoinHandle},
};

use ratatui::{
  crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
  },
  restore,
};

use super::{AppOptions, Builder, Renderer, Scanner};

/// Represent the application data
pub struct App {
  options: AppOptions,
  rules: Vec<Rule>,
  threads: VecDeque<JoinHandle<()>>,
}

impl App {
  /// Construct a new App instance
  pub fn new(options: AppOptions) -> Self {
    Self {
      options,
      threads: VecDeque::new(),
      rules: DEFAULT_RULES.clone(),
    }
  }

  /// Define the panic hook to restore the terminal to it's default state after panics
  fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
      Renderer::restore_terminal();
      Debug::log(format!("Panic {:?}", panic_info));
      hook(panic_info);
    }));
  }

  /// Run the whole application
  pub fn run(&mut self) -> crate::Result<()> {
    if let Some(path) = self.options.config_path.as_ref() {
      if self.options.eject_config {
        self.rules = init_rules(Some(path.clone()))?;
      } else {
        self.rules = load_rules(Some(path.clone()))?;
      }
    } else {
      self.rules = init_rules(None)?;
      if self.options.eject_config {
        let path = default_system_location()
          .as_ref()
          .and_then(|path| path.file_name())
          .and_then(|fname| fname.to_str())
          .map(|fname| PathBuf::from(format!("./{}", fname)))
          .unwrap();
        save_rules(&self.rules, Some(path))?;
      }
    }

    set_active_rule(&self.options.active_rule);

    if self.options.dump_rules {
      for r in &self.rules {
        println!("- Rule: {:?}", r.aliases);
        println!("  Command: {}", r.command);
        println!(
          "  Markers:\n{}",
          r.markers
            .iter()
            .map(|marker| format!("    {:?}: {}", marker.tag, marker.regex.as_str(),))
            .collect::<Vec<_>>()
            .join("\n")
        );
      }
      exit(0);
    }

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
