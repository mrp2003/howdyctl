//! Automated repairs for the issues [`crate::doctor`] reports — the things we'd
//! otherwise fix by hand. Every fix is **idempotent**: running it on a healthy
//! install changes nothing. All of these touch root-owned files, so the caller is
//! expected to already be root (`howdyctl` elevates `doctor --fix` via pkexec).
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Result of attempting one repair.
#[derive(Debug, Clone)]
pub struct FixOutcome {
    pub name: String,
    /// Something was actually changed (vs. already healthy).
    pub changed: bool,
    /// The repair succeeded (or nothing needed doing).
    pub ok: bool,
    pub message: String,
}

impl FixOutcome {
    fn done(name: &str, changed: bool, msg: impl Into<String>) -> Self {
        FixOutcome {
            name: name.into(),
            changed,
            ok: true,
            message: msg.into(),
        }
    }
    fn fail(name: &str, msg: impl Into<String>) -> Self {
        FixOutcome {
            name: name.into(),
            changed: false,
            ok: false,
            message: msg.into(),
        }
    }
}

/// Run every automated repair, in a safe order.
///
/// Not everything is auto-fixable: missing dlib/OpenCV (install packages) and a
/// missing face model (needs your face) are left to the user.
pub fn repair() -> Vec<FixOutcome> {
    let base = crate::base_dir();
    vec![
        fix_pam_python3(&base),
        fix_dirs_traversable(&base),
        fix_data_files(&base),
        fix_camera(&base),
        fix_pam_wired(),
    ]
}

/// Rewrite a Python-2 `import ConfigParser` as a 2/3-compatible shim.
/// Returns `None` if the text already imports `configparser` or has no bare import.
fn patch_pam_text(text: &str) -> Option<String> {
    if text.contains("configparser") {
        return None; // already Python 3 aware
    }
    const SHIM: &str = "try:\n    import configparser as ConfigParser\nexcept ImportError:\n    import ConfigParser";
    let mut replaced = false;
    let out: Vec<String> = text
        .lines()
        .map(|l| {
            if !replaced && l.trim() == "import ConfigParser" {
                replaced = true;
                SHIM.to_string()
            } else {
                l.to_string()
            }
        })
        .collect();
    if !replaced {
        return None;
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    Some(joined)
}

fn fix_pam_python3(base: &Path) -> FixOutcome {
    let name = "pam.py Python 3 import";
    let path = base.join("pam.py");
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => return FixOutcome::fail(name, format!("read {}: {e}", path.display())),
    };
    match patch_pam_text(&text) {
        None => FixOutcome::done(name, false, "already Python 3 compatible"),
        Some(patched) => match fs::write(&path, patched) {
            Ok(()) => FixOutcome::done(name, true, "patched to use configparser with a fallback"),
            Err(e) => FixOutcome::fail(name, format!("write {}: {e}", path.display())),
        },
    }
}

/// Ensure every directory in the Howdy tree is traversable (so `compare.py`, running
/// as the unprivileged user under PAM, can import from them).
fn fix_dirs_traversable(base: &Path) -> FixOutcome {
    let name = "Directories traversable";
    let mut dirs = Vec::new();
    collect_dirs(base, &mut dirs);
    let mut changed = 0;
    for d in dirs {
        let Ok(meta) = fs::metadata(&d) else { continue };
        let mode = meta.permissions().mode();
        let want = mode | 0o755; // at least rwxr-xr-x, keep any extra bits
        if want != mode {
            if let Err(e) = fs::set_permissions(&d, fs::Permissions::from_mode(want)) {
                return FixOutcome::fail(name, format!("chmod {}: {e}", d.display()));
            }
            changed += 1;
        }
    }
    if changed == 0 {
        FixOutcome::done(name, false, "all directories already traversable")
    } else {
        FixOutcome::done(
            name,
            true,
            format!("made {changed} director(ies) traversable"),
        )
    }
}

fn collect_dirs(dir: &Path, out: &mut Vec<PathBuf>) {
    out.push(dir.to_path_buf());
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            // file_type() does not follow symlinks, so we won't wander out of the tree
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    collect_dirs(&entry.path(), out);
                }
            }
        }
    }
}

fn fix_data_files(base: &Path) -> FixOutcome {
    let name = "dlib model data";
    let files = [
        "dlib-data/shape_predictor_5_face_landmarks.dat",
        "dlib-data/dlib_face_recognition_resnet_model_v1.dat",
        "dlib-data/mmod_human_face_detector.dat",
    ];
    if files.iter().all(|f| base.join(f).exists()) {
        return FixOutcome::done(name, false, "all data files present");
    }
    let script = base.join("dlib-data/install.sh");
    if !script.exists() {
        return FixOutcome::fail(
            name,
            "data files missing and dlib-data/install.sh not found",
        );
    }
    let status = Command::new("bash")
        .arg(&script)
        .current_dir(base.join("dlib-data"))
        .status();
    match status {
        Ok(s) if s.success() => FixOutcome::done(name, true, "downloaded the model data files"),
        Ok(s) => FixOutcome::fail(
            name,
            format!("install.sh exited {}", s.code().unwrap_or(-1)),
        ),
        Err(e) => FixOutcome::fail(name, format!("running install.sh: {e}")),
    }
}

fn fix_camera(base: &Path) -> FixOutcome {
    let name = "Camera device_path";
    let _ = base;
    let mut cfg = match crate::Config::load() {
        Ok(c) => c,
        Err(e) => return FixOutcome::fail(name, format!("read config: {e}")),
    };
    let cur = cfg.get("device_path").unwrap_or_default();
    let cams = crate::camera::detect();
    let cur_ok = !cur.is_empty()
        && cur != "none"
        && cams
            .iter()
            .any(|c| c.path.to_string_lossy() == cur && c.can_capture);
    if cur_ok {
        return FixOutcome::done(name, false, format!("already set to {cur}"));
    }
    match crate::camera::capture_devices().into_iter().next() {
        Some(cam) => {
            let path = cam.path.to_string_lossy().into_owned();
            cfg.set("device_path", &path);
            match cfg.save() {
                Ok(()) => FixOutcome::done(
                    name,
                    true,
                    format!("set to {path} ({})", if cam.is_ir { "IR" } else { "RGB" }),
                ),
                Err(e) => FixOutcome::fail(name, format!("write config: {e}")),
            }
        }
        None => FixOutcome::fail(name, "no capture-capable camera found"),
    }
}

fn fix_pam_wired() -> FixOutcome {
    let name = "PAM wired in";
    let is_wired = || {
        fs::read_to_string("/etc/pam.d/common-auth")
            .map(|t| {
                t.lines()
                    .any(|l| !l.trim_start().starts_with('#') && l.contains("howdy"))
            })
            .unwrap_or(false)
    };
    if is_wired() {
        return FixOutcome::done(name, false, "already present in common-auth");
    }
    let status = Command::new("pam-auth-update").arg("--package").status();
    match status {
        Ok(s) if s.success() => {
            if is_wired() {
                FixOutcome::done(name, true, "enabled via pam-auth-update")
            } else {
                FixOutcome::fail(name, "pam-auth-update ran but Howdy is still not enabled")
            }
        }
        Ok(s) => FixOutcome::fail(
            name,
            format!("pam-auth-update exited {}", s.code().unwrap_or(-1)),
        ),
        Err(e) => FixOutcome::fail(name, format!("running pam-auth-update: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::patch_pam_text;

    #[test]
    fn patches_bare_python2_import() {
        let src = "import os\nimport ConfigParser\nconfig = ConfigParser.ConfigParser()\n";
        let out = patch_pam_text(src).expect("should patch");
        assert!(out.contains("import configparser as ConfigParser"));
        assert!(out.contains("except ImportError:"));
        assert!(out.ends_with('\n'));
        // the consumer of the alias is untouched
        assert!(out.contains("config = ConfigParser.ConfigParser()"));
    }

    #[test]
    fn leaves_already_fixed_file_alone() {
        let src = "try:\n    import configparser as ConfigParser\nexcept ImportError:\n    import ConfigParser\n";
        assert!(patch_pam_text(src).is_none());
    }

    #[test]
    fn no_change_when_no_configparser_import() {
        assert!(patch_pam_text("import os\nimport sys\n").is_none());
    }
}
