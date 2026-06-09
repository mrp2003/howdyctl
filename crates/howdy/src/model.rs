//! List, enroll and remove a user's face models.
//!
//! Listing reads the model file directly (it's world-readable JSON), so it works
//! without root. Enrolling and removing shell out to Howdy's own CLI and therefore
//! require root — callers are expected to have elevated already.
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::Deserialize;

/// One enrolled face model. The large `data` encoding array in the file is ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct Model {
    pub id: u32,
    pub label: String,
    /// Unix timestamp the model was enrolled.
    pub time: i64,
}

/// Path to a user's model file, e.g. `…/models/pishu.dat`.
fn model_path(user: &str) -> PathBuf {
    crate::base_dir().join("models").join(format!("{user}.dat"))
}

/// List a user's enrolled models (empty if none / file absent). Root not required.
pub fn list(user: &str) -> anyhow::Result<Vec<Model>> {
    let path = model_path(user);
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(anyhow::Error::new(e).context(format!("reading {}", path.display()))),
    };
    let models: Vec<Model> = serde_json::from_str(&data)
        .map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))?;
    Ok(models)
}

/// Format a model timestamp as `YYYY-MM-DD HH:MM` (UTC).
pub fn format_time(epoch: i64) -> String {
    let t = epoch as libc::time_t;
    // SAFETY: gmtime returns a pointer to a static `tm`; we copy it out at once.
    unsafe {
        let tm = libc::gmtime(&t);
        if tm.is_null() {
            return epoch.to_string();
        }
        let tm = *tm;
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min
        )
    }
}

/// Path to Howdy's Python CLI, invoked directly to dodge `$PATH` surprises under
/// `pkexec` (which would otherwise miss `/usr/local/bin/howdy`).
fn cli_py() -> PathBuf {
    crate::base_dir().join("cli.py")
}

/// Enroll a new face model for `user` (root required). Inherits stdio so the user
/// sees the camera prompts; a `label` is fed on stdin, otherwise Howdy's default
/// label is used (`-y`).
pub fn add(user: &str, label: Option<&str>) -> anyhow::Result<()> {
    let cli = cli_py();
    let mut cmd = Command::new("python3");
    cmd.arg(&cli).arg("-U").arg(user);

    let status = match label {
        Some(l) => {
            cmd.arg("add").stdin(Stdio::piped());
            let mut child = cmd.spawn()?;
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                writeln!(stdin, "{l}")?;
            }
            child.wait()?
        }
        None => cmd.arg("-y").arg("add").status()?,
    };

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "enrollment failed (exit {})",
            status.code().unwrap_or(-1)
        ))
    }
}

/// Remove a single model by id (root required).
pub fn remove(user: &str, id: u32) -> anyhow::Result<()> {
    run_cli(user, &["-y", "remove", &id.to_string()], "removing model")
}

/// Remove all of a user's models (root required).
pub fn clear(user: &str) -> anyhow::Result<()> {
    run_cli(user, &["-y", "clear"], "clearing models")
}

fn run_cli(user: &str, args: &[&str], what: &str) -> anyhow::Result<()> {
    let cli = cli_py();
    let status = Command::new("python3")
        .arg(&cli)
        .arg("-U")
        .arg(user)
        .args(args)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "{what} failed (exit {})",
            status.code().unwrap_or(-1)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_model_metadata_ignoring_encoding() {
        let json = r#"[{"time": 1781031338, "label": "Initial model", "id": 0,
            "data": [[-0.094, 0.029, 0.048]]}]"#;
        let models: Vec<Model> = serde_json::from_str(json).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, 0);
        assert_eq!(models[0].label, "Initial model");
        assert_eq!(models[0].time, 1781031338);
    }

    #[test]
    fn formats_time_as_utc() {
        // 1781031338 == 2026-06-09 18:55:38 UTC (gmtime, so CI is timezone-independent)
        assert_eq!(format_time(1781031338), "2026-06-09 18:55");
    }
}
