use std::io::{stdin, IsTerminal as _};

/// Represent the application options
#[derive(Default, Clone)]
pub struct AppOptions {
  pub stdin: bool,
  pub build_args: Vec<String>,
}

impl AppOptions {
  /// Parse command line to extract options
  pub fn parse(mut self) -> Self {
    if !stdin().is_terminal() {
      self.stdin = true
    }
    self.build_args = std::env::args().skip(1).collect::<Vec<_>>();
    if self.build_args.len() != 0 && self.stdin {
      panic!("Cannot have both stdin content and command-line arguments")
    }
    self
  }
}
