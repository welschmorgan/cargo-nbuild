use std::{
  collections::HashMap,
  io::{stderr, stdout, Read, Write},
  path::{Path, PathBuf},
  process::exit,
  sync::{Arc, Mutex, MutexGuard},
};

use dirs::{config_dir, config_local_dir, data_dir, data_local_dir, state_dir};
use lazy_static::lazy_static;
use ratatui::style::{Style, Stylize as _};
use regex::Regex;
use serde::{
  ser::{SerializeSeq, SerializeStruct as _},
  Deserialize, Serialize,
};

use crate::{err, search, DeclaredMarker, ErrorKind};

use super::BuildTagKind;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Rule {
  pub aliases: Vec<String>,
  pub command: String,
  pub markers: Vec<DeclaredMarker>,
}

impl Rule {
  pub fn new<
    A: AsRef<str>,
    AA: IntoIterator<Item = A>,
    M: IntoIterator<Item = (BuildTagKind, Regex, Style)>,
    C: AsRef<str>,
  >(
    aliases: AA,
    command: C,
    markers: M,
  ) -> Self {
    Self {
      aliases: aliases
        .into_iter()
        .map(|alias| alias.as_ref().to_string())
        .collect::<Vec<_>>(),
      command: command.as_ref().to_string(),
      markers: Vec::from_iter(
        markers
          .into_iter()
          .map(|(tag, regex, style)| DeclaredMarker::new(tag, regex, style)),
      ),
    }
  }
}

pub const CONFIG_BASE_NAME: &'static str = "nbuild";

pub type RuleLoader = dyn Fn(Box<dyn Read>) -> crate::Result<Vec<Rule>>;
pub type RuleSaver = dyn Fn(&Vec<Rule>, Box<dyn Write>) -> crate::Result<()>;

pub struct RuleFormat {
  exts: Vec<String>,
  deserialize: Box<RuleLoader>,
  serialize: Box<RuleSaver>,
}

unsafe impl Sync for RuleFormat {}
unsafe impl Send for RuleFormat {}

impl RuleFormat {
  pub fn new<
    X: AsRef<str>,
    I: IntoIterator<Item = X>,
    D: Fn(Box<dyn Read>) -> crate::Result<Vec<Rule>> + 'static,
    S: Fn(&Vec<Rule>, Box<dyn Write>) -> crate::Result<()> + 'static,
  >(
    exts: I,
    deserialize: D,
    serialize: S,
  ) -> Self {
    Self {
      exts: exts
        .into_iter()
        .map(|item| item.as_ref().to_string())
        .collect::<Vec<_>>(),
      deserialize: Box::new(deserialize),
      serialize: Box::new(serialize),
    }
  }
}

lazy_static! {
  pub static ref RULE_FORMATS: Vec<RuleFormat> = vec![
    #[cfg(feature = "json")]
    RuleFormat::new(
      ["json".to_string()],
      |r| {
        serde_json::from_reader(r)
          .map_err(|e| err!(ErrorKind::IO, "failed to read json from stream, {}", e))
      },
      |rules, w| {
        serde_json::to_writer_pretty(w, rules)
          .map_err(|e| err!(ErrorKind::IO, "failed to write json to stream, {}", e))
      }
    ),
    #[cfg(feature = "yaml")]
    RuleFormat::new(
      ["yaml".to_string(), "yml".to_string()],
      |r| {
        serde_yml::from_reader(r)
          .map_err(|e| err!(ErrorKind::IO, "failed to read yaml from stream, {}", e))
      },
      |rules, w| {
        serde_yml::to_writer(w, rules)
          .map_err(|e| err!(ErrorKind::IO, "failed to write json to stream, {}", e))
      }
    ),
    #[cfg(feature = "toml")]
    RuleFormat::new(
      ["toml".to_string()],
      |mut r| {
        let mut buf = String::new();
        r.read_to_string(&mut buf)
          .map_err(|e| err!(ErrorKind::IO, "failed to read yaml from stream, {}", e))?;
        toml::from_str(&buf)
          .map_err(|e| err!(ErrorKind::IO, "failed to read yaml from stream, {}", e))
      },
      |rules, mut w| {
        let content = toml::to_string(rules)
          .map_err(|e| err!(ErrorKind::IO, "failed to serialize toml, {}", e))?;
        let _ = w
          .write(content.as_bytes())
          .map_err(|e| err!(ErrorKind::IO, "failed to write toml to stream, {}", e))?;
        Ok(())
      }
    ),
  ];
  pub static ref DEFAULT_RULES: Vec<Rule> = vec![Rule::new(
    ["rust: cargo", "cargo", "rust"],
    "cargo build",
    [
      (
        BuildTagKind::Error,
        Regex::new(r"error(\[\w+\])?:").expect("invalid regular expression"),
        Style::default().red().bold()
      ),
      (
        BuildTagKind::Note,
        Regex::new(r"note(\[\w+\])?:").expect("invalid regular expression"),
        Style::default().blue().bold()
      ),
      (
        BuildTagKind::Warning,
        Regex::new(r"warning(\[\w+\])?:").expect("invalid regular expression"),
        Style::default().yellow().bold()
      ),
    ]
  )];
  static ref _rules: Arc<Mutex<Vec<Rule>>> = Arc::new(Mutex::new(DEFAULT_RULES.clone()));
  static ref _active_rule: Arc<Mutex<String>> = Arc::new(Mutex::new("rust".to_string()));
}

pub fn rules() -> Vec<Rule> {
  let g = _rules.lock().expect("failed to lock rules");
  g.clone()
}

pub fn set_active_rule<S: AsRef<str>>(s: S) {
  match rule(s.as_ref()) {
    None => panic!("unknown rule '{}'", s.as_ref()),
    Some(_) => {
      crate::dbg!("Active rule is now {:?}", s.as_ref());
      let mut g = _active_rule.lock().expect("failed to lock active rule");
      g.clear();
      g.push_str(s.as_ref());
    }
  }
}

pub fn active_rule_name() -> String {
  let a = _active_rule.lock().expect("failed to lock active rule");
  a.clone()
}

pub fn rule<S: AsRef<str>>(s: S) -> Option<Rule> {
  let rules = rules();
  rules
    .iter()
    .find(|rule| {
      rule
        .aliases
        .iter()
        .find(|alias| alias.eq_ignore_ascii_case(s.as_ref()))
        .is_some()
    })
    .cloned()
}

pub fn active_rule<'a>() -> Rule {
  let rules: MutexGuard<'a, Vec<Rule>> = _rules.lock().expect("failed to lock rules");
  let a: MutexGuard<'a, String> = _active_rule.lock().expect("failed to lock active rule");
  rules
    .iter()
    .find(|rule| {
      rule
        .aliases
        .iter()
        .find(|alias| alias.to_lowercase().eq(a.as_str()))
        .is_some()
    })
    .expect("invalid active rule")
    .clone()
}

pub fn default_system_location() -> Option<PathBuf> {
  if let Some(dir) = config_dir() {
    let dir = PathBuf::from(format!("{}", dir.display()).replace("\\", "/"));
    return search_locations()
      .iter()
      .find(|loc| loc.starts_with(&dir))
      .cloned();
  }
  None
}

pub fn search_locations() -> Vec<PathBuf> {
  vec![
    std::env::current_dir().ok(),
    config_local_dir()
      .map(|dir| PathBuf::from(format!("{}/{}", dir.display(), env!("CARGO_PKG_NAME")))),
    config_dir().map(|dir| PathBuf::from(format!("{}/{}", dir.display(), env!("CARGO_PKG_NAME")))),
    state_dir().map(|dir| PathBuf::from(format!("{}/{}", dir.display(), env!("CARGO_PKG_NAME")))),
    data_local_dir()
      .map(|dir| PathBuf::from(format!("{}/{}", dir.display(), env!("CARGO_PKG_NAME")))),
    data_dir().map(|dir| PathBuf::from(format!("{}/{}", dir.display(), env!("CARGO_PKG_NAME")))),
    #[cfg(target_os = "linux")]
    Some(PathBuf::from("/usr/share/nbuild")),
  ]
  .iter()
  .filter(|dir| dir.is_some())
  .flat_map(|dir| {
    RULE_FORMATS
      .iter()
      .flat_map(|fmt| {
        fmt.exts.iter().map(|ext| {
          format!(
            "{}/{}.{}",
            dir.as_ref().unwrap().display(),
            CONFIG_BASE_NAME,
            ext
          )
        })
      })
      .map(|path| path.replace("\\", "/"))
      .map(|path| PathBuf::from(path))
      .collect::<Vec<_>>()
  })
  .collect::<Vec<_>>()
}

pub fn locate_rules<'a>() -> Option<(PathBuf, &'a RuleFormat)> {
  if let Some(loc) = search_locations()
    .iter()
    .find(|path| path.exists())
    .cloned()
  {
    if let Some(path_ext) = loc.extension().and_then(|ext| ext.to_str()) {
      let path_ext = path_ext.to_lowercase();
      let fmt = RULE_FORMATS.iter().find(|fmt| {
        fmt
          .exts
          .iter()
          .find(|ext| ext.to_lowercase().as_str().eq(path_ext.as_str()))
          .is_some()
      });
      if let Some(fmt) = fmt {
        return Some((loc, fmt));
      }
    }
  }
  None
}

pub fn find_format<'a, P: AsRef<Path>>(path: P) -> Option<&'a RuleFormat> {
  path
    .as_ref()
    .extension()
    .and_then(|ext| ext.to_str())
    .and_then(|ext| Some(ext.to_lowercase()))
    .and_then(|path_ext| {
      RULE_FORMATS.iter().find(|fmt| {
        fmt
          .exts
          .iter()
          .find(|ext| ext.to_lowercase().as_str().eq(path_ext.as_str()))
          .is_some()
      })
    })
}

pub fn load_rules(custom_path: Option<PathBuf>) -> crate::Result<Vec<Rule>> {
  let path = custom_path
    .and_then(|p| find_format(&p).and_then(|fmt| Some((p, fmt))))
    .or_else(|| locate_rules());
  match path {
    Some((loc, fmt)) => {
      crate::dbg!("Loading rules from {}", loc.display());
      if let Ok(f) = std::fs::File::open(&loc) {
        let rules = (fmt.deserialize)(Box::new(f))?;
        let mut g = _rules.lock().expect("failed to lock rules");
        let existing_rule_names = g
          .iter()
          .flat_map(|r| r.aliases.iter().map(|alias| alias.to_lowercase()))
          .collect::<Vec<_>>();
        for r in &rules {
          if r
            .aliases
            .iter()
            .find(|alias| existing_rule_names.contains(&alias.to_lowercase()))
            .is_none()
          {
            g.push(r.clone());
          }
        }
        for rule in g.iter() {
          crate::dbg!("Found rule {:?}", rule.aliases);
        }
        return Ok(g.clone());
      }
      Err(err!(
        ErrorKind::IO,
        "failed to open file for reading {}",
        loc.display()
      ))
    }
    None => Err(err!(ErrorKind::FileNotFound)),
  }
}

pub fn save_rules(rules: &Vec<Rule>, custom_path: Option<PathBuf>) -> crate::Result<PathBuf> {
  let path = custom_path
    .and_then(|p| find_format(&p).and_then(|fmt| Some((p, fmt))))
    .or_else(|| locate_rules());
  if let Some((loc, fmt)) = match path {
    Some((loc, fmt)) => Some((loc, fmt)),
    None => default_system_location().map(|loc| (loc, &RULE_FORMATS[0])),
  } {
    crate::dbg!("Saving {} rules to {}", rules.len(), loc.display());
    if let Some(parent) = loc.parent() {
      if !parent.try_exists().map_err(|e| {
        err!(
          ErrorKind::IO,
          "failed to check for dir existence {}, {}",
          parent.display(),
          e
        )
      })? {
        crate::dbg!("Creating folder {}", parent.display());
        let _ = std::fs::create_dir_all(parent);
      }
    }
    match std::fs::File::create(&loc) {
      Ok(f) => {
        (fmt.serialize)(rules, Box::new(f))?;
        return Ok(loc);
      }
      Err(e) => {
        return Err(err!(
          ErrorKind::IO,
          "failed to open file for writing {}, {}",
          loc.display(),
          e
        ))
      }
    }
  }
  Err(err!(
    ErrorKind::IO,
    "failed to find config location or format"
  ))
}

pub fn init_rules(custom_path: Option<PathBuf>) -> crate::Result<Vec<Rule>> {
  match load_rules(custom_path.clone()) {
    Ok(rules) => return Ok(rules),
    Err(eload) => match save_rules(&DEFAULT_RULES, custom_path.clone()) {
      Ok(_) => return load_rules(custom_path),
      Err(esave) => Err(err!(
        ErrorKind::IO,
        "failed to initialize rules:\n  - {}\n  - {}",
        eload,
        esave
      )),
    },
  }
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use dirs::config_dir;

  use crate::{rule::RULE_FORMATS, CONFIG_BASE_NAME};

  #[test]
  fn search_locations() {
    let locs = super::search_locations();
    println!(
      "Locations: {:#?}",
      locs.iter().map(|loc| loc.display()).collect::<Vec<_>>()
    );
    // always at least the current working dir
    assert!(locs.len() >= 1);
    assert_eq!(
      locs[0],
      PathBuf::from(
        format!(
          "{}/{}.{}",
          std::env::current_dir().expect("cur working dir").display(),
          CONFIG_BASE_NAME,
          RULE_FORMATS[0].exts[0]
        )
        .replace("\\", "/")
      )
    );
  }

  #[test]
  fn default_system_location() {
    let loc = super::default_system_location();
    assert_eq!(
      loc,
      Some(PathBuf::from(format!(
        "{}/{}/{}.{}",
        config_dir().unwrap().display(),
        env!("CARGO_PKG_NAME"),
        CONFIG_BASE_NAME,
        RULE_FORMATS[0].exts[0]
      )))
    );
  }
}
