use std::process::ExitStatus;

/// Represent a cargo build event
#[derive(Debug, Clone, Copy)]
pub enum BuildEvent {
  /// Cargo process spawned
  BuildStarted,
  /// Cargo process finished
  BuildFinished(ExitStatus),
  /// Compilation error detected
  BuildError(usize),
}
