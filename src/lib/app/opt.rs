use std::io::{stdin, IsTerminal as _};

/// Represent the application options
#[derive(Default, Clone, Debug)]
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
    let pkg_name = env!("CARGO_PKG_NAME").replace("cargo-", "");
    self.build_args = std::env::args().skip(1).collect::<Vec<_>>();
    if !self.build_args.is_empty() && self.build_args[0].eq(&pkg_name) {
      self.build_args.remove(0);
    }
    if self.build_args.len() != 0 && self.stdin {
      panic!("Cannot have both stdin content and command-line arguments")
    }
    self
  }
}
