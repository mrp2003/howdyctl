//! # howdy
//!
//! A small, dependency-light Rust library for inspecting and driving a
//! [Howdy](https://github.com/boltgolt/howdy) face-authentication install on Linux:
//! enumerate cameras (telling IR apart from RGB), read and write `config.ini`,
//! list / enroll / remove face models, run a recognition test that reports the
//! actual *match distance*, and run health checks.
//!
//! It powers the [`howdyctl`](https://github.com/mrp2003/howdyctl) TUI, and is the
//! sibling of [`lamparray`](https://crates.io/crates/lamparray) — both born on an
//! **ASUS TUF** laptop whose hardware Linux supports only if you talk to it directly.
//!
//! Operations that only *read* state (camera detection, [`model::list`],
//! [`test::run`], [`doctor::run`]) work as a normal user. Operations that *mutate*
//! Howdy's root-owned files (enroll/remove models, edit config) must run as root;
//! `howdyctl` elevates those with `pkexec`.
#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;

pub mod camera;
pub mod config;
pub mod doctor;
mod ioctl;
pub mod model;
pub mod test;

pub use camera::Camera;
pub use config::Config;
pub use doctor::{Check, Status};
pub use model::Model;
pub use test::TestResult;

/// Candidate locations for the Howdy install (`/lib` is a symlink to `/usr/lib`
/// on usrmerge systems, but both are worth checking explicitly).
const BASE_CANDIDATES: [&str; 2] = ["/lib/security/howdy", "/usr/lib/security/howdy"];

/// Locate the installed Howdy base directory (the one containing `config.ini`).
///
/// Falls back to the conventional path if nothing is found, so callers can still
/// build sensible error messages.
pub fn base_dir() -> PathBuf {
    for p in BASE_CANDIDATES {
        let pb = PathBuf::from(p);
        if pb.join("config.ini").exists() {
            return pb;
        }
    }
    PathBuf::from(BASE_CANDIDATES[0])
}

/// Whether Howdy looks installed (its `config.ini` exists).
pub fn is_installed() -> bool {
    base_dir().join("config.ini").exists()
}

/// Best guess at the login name to operate on.
///
/// Honours `SUDO_USER` / `PKEXEC_UID` so that when we are elevated we still target
/// the human who launched us rather than `root`.
pub fn current_user() -> String {
    if let Ok(u) = std::env::var("SUDO_USER") {
        if !u.is_empty() && u != "root" {
            return u;
        }
    }
    if let Ok(uid) = std::env::var("PKEXEC_UID") {
        if let Ok(uid) = uid.parse::<u32>() {
            if let Some(name) = username_for_uid(uid) {
                return name;
            }
        }
    }
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "root".to_string())
}

/// Resolve a uid to a login name via `getpwuid`.
fn username_for_uid(uid: u32) -> Option<String> {
    // SAFETY: getpwuid returns a pointer to a static buffer; we copy out of it
    // immediately and never hold it across another libc call.
    unsafe {
        let pw = libc::getpwuid(uid as libc::uid_t);
        if pw.is_null() {
            return None;
        }
        let name = (*pw).pw_name;
        if name.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr(name)
                .to_string_lossy()
                .into_owned(),
        )
    }
}
