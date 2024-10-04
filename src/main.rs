use std::process::{ExitCode, ExitStatus};

use cargo_nbuild::{App, AppOptions, Result};

fn main() -> ExitCode {
  let opt = AppOptions::default().parse();
  if let Err(e) = App::new(opt).run() {
    eprintln!("\x1b[0;31mfatal\x1b[0m: {}", e);
    if let Some(loc) = e.location() {
      eprintln!("-> \x1b[0;34mat\x1b[0m: {}", loc);
    }
    return ExitCode::FAILURE;
  }
  ExitCode::SUCCESS
}
