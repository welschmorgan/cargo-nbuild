use std::{
  collections::HashMap,
  io::{stdin, IsTerminal as _},
  process::exit,
};

use lazy_static::lazy_static;

struct KnownOption {
  name: String,
  long: Option<String>,
  short: Option<char>,
  desc: Option<String>,
  needs_value: bool,
  activate: Option<Box<dyn Fn(&mut AppOptions, Option<String>)>>,
}

unsafe impl Send for KnownOption {}
unsafe impl Sync for KnownOption {}

impl KnownOption {
  pub fn new<N: AsRef<str>>(name: N) -> Self {
    Self {
      name: name.as_ref().to_string(),
      long: None,
      short: None,
      activate: None,
      needs_value: false,
      desc: None,
    }
  }

  pub fn with_long<S: AsRef<str>>(mut self, s: S) -> Self {
    self.long = Some(s.as_ref().to_string());
    self
  }

  pub fn with_short(mut self, ch: char) -> Self {
    self.short = Some(ch);
    self
  }

  pub fn with_desc<D: AsRef<str>>(mut self, v: D) -> Self {
    self.desc = Some(v.as_ref().to_string());
    self
  }

  pub fn with_value_required(mut self, v: bool) -> Self {
    self.needs_value = v;
    self
  }

  pub fn with_activate<F: Fn(&mut AppOptions, Option<String>) + 'static>(mut self, f: F) -> Self {
    self.activate = Some(Box::new(f));
    self
  }
}

lazy_static! {
  static ref KNOWN_OPTIONS: Vec<KnownOption> = vec![
    KnownOption::new("help")
      .with_long("--help")
      .with_short('h')
      .with_activate(|opts, arg| opts.show_help = true)
      .with_desc("Show this help screen"),
    KnownOption::new("only-errors")
      .with_long("--only-errors")
      .with_short('E')
      .with_activate(|opts, arg| opts.show_only_errors = true)
      .with_desc("Filter logs: show only errors"),
  ];
}

/// Represent the application options
#[derive(Default, Clone, Debug)]
pub struct AppOptions {
  pub stdin: bool,
  pub show_help: bool,
  pub show_only_errors: bool,
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
    let mut i: isize = 0;
    while i < self.build_args.len() as isize {
      let arg = self.build_args[i as usize].clone();
      if arg.eq("--") {
        self.build_args.remove(i as usize);
        break;
      }
      if let Some(known_opt) = KNOWN_OPTIONS.iter().find(|opt| {
        if opt.long.is_some() && opt.long.as_ref().unwrap().eq_ignore_ascii_case(&arg) {
          return true;
        }
        if opt.short.is_some() && arg.eq(&format!("-{}", opt.short.as_ref().unwrap())) {
          return true;
        }
        return false;
      }) {
        self.build_args.remove(i as usize);
        let mut arg = None;
        if known_opt.needs_value {
          arg = Some(self.build_args[i as usize].clone());
          self.build_args.remove(i as usize);
        }
        known_opt.activate.as_ref().unwrap()(&mut self, arg);
        i -= 1;
      }
      i += 1;
    }
    if self.show_help {
      Self::usage();
    }
    crate::dbg!("{:#?}", self);
    self
  }

  fn usage() {
    eprintln!(
      "\x1b[90musage:\x1b[0m \x1b[1m{}\x1b[0m [OPTIONS...]",
      env!("CARGO_PKG_NAME")
    );
    eprintln!();
    eprintln!(env!("CARGO_PKG_DESCRIPTION"));
    eprintln!();
    eprintln!("\x1b[1m* Options\x1b[0m");
    eprintln!();
    let mut opts = vec![];
    let mut widths = [0 as usize; 2];
    for opt in KNOWN_OPTIONS.iter() {
      let opt_s = format!(
        "{}{}{}",
        match opt.short {
          Some(ch) => format!("-{}", ch),
          None => String::new(),
        },
        match opt.long.as_ref() {
          Some(long) => match opt.short {
            Some(_) => format!(", {}", long),
            None => long.clone(),
          },
          None => String::new(),
        },
        match opt.needs_value {
          true => " <VALUE>",
          false => "",
        }
      );
      let desc = opt.desc.clone().unwrap_or_default();
      widths[0] = widths[0].max(opt_s.len());
      widths[1] = widths[1].max(desc.len());
      opts.push([opt_s, desc]);
    }
    for opt in opts {
      eprintln!("  {:width$}: {}", opt[0], opt[1], width = widths[0])
    }
    eprintln!();
    eprintln!("\x1b[1m* Author\x1b[0m");
    eprintln!();
    for author in env!("CARGO_PKG_AUTHORS").split(":") {
      eprintln!("  {}", author);
    }
    exit(0);
  }
}
