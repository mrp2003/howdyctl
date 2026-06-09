//! Run privileged actions by re-executing ourselves under `pkexec`.
//!
//! Howdy's models and config are root-owned, so enroll/remove/config-write actions
//! need privilege. Rather than run the whole TUI as root, we elevate just the one
//! action: spawn `pkexec <our binary> <args>` (or run directly if already root).
use std::io;
use std::process::{Command, ExitStatus};

/// Are we running as root?
pub fn is_root() -> bool {
    // SAFETY: geteuid is always safe to call.
    unsafe { libc::geteuid() == 0 }
}

/// Run `howdyctl <args>` with privilege, inheriting stdio, and wait for it.
///
/// When already root, runs the action in a child directly; otherwise goes through
/// `pkexec` (which pops the system's polkit prompt). The child re-enters `main`,
/// sees it is root, and performs the action.
pub fn run(args: &[String]) -> io::Result<ExitStatus> {
    let exe = std::env::current_exe()?;
    if is_root() {
        Command::new(exe).args(args).status()
    } else {
        Command::new("pkexec").arg(exe).args(args).status()
    }
}
