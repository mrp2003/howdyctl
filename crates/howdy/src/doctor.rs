//! Health checks for a Howdy install — the exact things that commonly break it on
//! modern distros (Python 2 leftovers in `pam.py`, non-traversable directories under
//! PAM, missing dlib, broken PAM wiring, an inaccessible camera).
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

/// Severity of a check result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

/// A single health check.
#[derive(Debug, Clone)]
pub struct Check {
    pub name: String,
    pub status: Status,
    pub detail: String,
}

impl Check {
    fn new(name: &str, status: Status, detail: impl Into<String>) -> Check {
        Check {
            name: name.to_string(),
            status,
            detail: detail.into(),
        }
    }
}

/// Run every health check for `user`.
pub fn run(user: &str) -> Vec<Check> {
    let base = crate::base_dir();
    vec![
        check_installed(&base),
        check_python_module("dlib", "dlib (face recognition)"),
        check_python_module("cv2", "OpenCV (cv2)"),
        check_data_files(&base),
        check_pam_python3(&base),
        check_dirs_traversable(&base),
        check_pam_wired(),
        check_camera(),
        check_models(user),
    ]
}

fn check_installed(base: &Path) -> Check {
    if base.join("config.ini").exists() {
        Check::new("Howdy installed", Status::Ok, base.display().to_string())
    } else {
        Check::new(
            "Howdy installed",
            Status::Fail,
            "no config.ini found under /lib/security/howdy",
        )
    }
}

fn check_python_module(module: &str, label: &str) -> Check {
    let ok = Command::new("python3")
        .arg("-c")
        .arg(format!("import {module}"))
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        Check::new(label, Status::Ok, "importable under python3")
    } else {
        Check::new(
            label,
            Status::Fail,
            format!("`python3 -c 'import {module}'` failed"),
        )
    }
}

fn check_data_files(base: &Path) -> Check {
    let files = [
        "dlib-data/shape_predictor_5_face_landmarks.dat",
        "dlib-data/dlib_face_recognition_resnet_model_v1.dat",
        "dlib-data/mmod_human_face_detector.dat",
    ];
    let missing: Vec<&str> = files
        .iter()
        .filter(|f| !base.join(f).exists())
        .copied()
        .collect();
    if missing.is_empty() {
        Check::new("dlib model data", Status::Ok, "all 3 data files present")
    } else {
        Check::new(
            "dlib model data",
            Status::Fail,
            format!("missing: {}", missing.join(", ")),
        )
    }
}

/// The bug that silently falls back to a password: `pam.py` using the Python 2
/// `ConfigParser` module name on a Python 3 pam-python.
fn check_pam_python3(base: &Path) -> Check {
    let path = base.join("pam.py");
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            // The bug is a top-level Python-2 `import ConfigParser` with no fallback to
            // the Python-3 `configparser`. A `try/except` shim that pulls in lowercase
            // `configparser` (as we install) is fine.
            let imports_py3 = text.contains("configparser");
            let imports_py2 = text.lines().any(|l| {
                let t = l.trim_start();
                !t.starts_with('#') && t.starts_with("import ConfigParser")
            });
            let bad = imports_py2 && !imports_py3;
            if bad {
                Check::new(
                    "pam.py Python 3 compatible",
                    Status::Fail,
                    "imports the Python 2 'ConfigParser' module — auth will fall through to password",
                )
            } else {
                Check::new(
                    "pam.py Python 3 compatible",
                    Status::Ok,
                    "no Python 2 imports",
                )
            }
        }
        Err(_) => Check::new(
            "pam.py Python 3 compatible",
            Status::Warn,
            format!("could not read {}", path.display()),
        ),
    }
}

/// Under PAM, `compare.py` runs as the unprivileged user; every directory it imports
/// from needs the world-execute (traverse) bit.
fn check_dirs_traversable(base: &Path) -> Check {
    let dirs = ["recorders", "dlib-data", "models"];
    let mut blocked = Vec::new();
    for d in dirs {
        let p = base.join(d);
        if let Ok(meta) = std::fs::metadata(&p) {
            if meta.permissions().mode() & 0o001 == 0 {
                blocked.push(d);
            }
        }
    }
    if blocked.is_empty() {
        Check::new(
            "Directories traversable",
            Status::Ok,
            "world-execute bit set",
        )
    } else {
        Check::new(
            "Directories traversable",
            Status::Fail,
            format!("not traversable by your user: {}", blocked.join(", ")),
        )
    }
}

fn check_pam_wired() -> Check {
    match std::fs::read_to_string("/etc/pam.d/common-auth") {
        Ok(text) => {
            let wired = text
                .lines()
                .any(|l| !l.trim_start().starts_with('#') && l.contains("howdy"));
            if wired {
                Check::new(
                    "PAM wired in",
                    Status::Ok,
                    "present in /etc/pam.d/common-auth",
                )
            } else {
                Check::new(
                    "PAM wired in",
                    Status::Warn,
                    "not found in common-auth (run `pam-auth-update`)",
                )
            }
        }
        Err(_) => Check::new(
            "PAM wired in",
            Status::Warn,
            "could not read /etc/pam.d/common-auth",
        ),
    }
}

fn check_camera() -> Check {
    let Ok(cfg) = crate::Config::load() else {
        return Check::new(
            "Camera configured",
            Status::Warn,
            "could not read config.ini",
        );
    };
    let Some(path) = cfg.get("device_path") else {
        return Check::new("Camera configured", Status::Fail, "device_path is unset");
    };
    if path == "none" || path.is_empty() {
        return Check::new("Camera configured", Status::Fail, "device_path = none");
    }
    let cam = crate::camera::detect()
        .into_iter()
        .find(|c| c.path.to_string_lossy() == path);
    match cam {
        Some(c) if c.can_capture && c.accessible => Check::new(
            "Camera configured",
            Status::Ok,
            format!("{} ({}{})", path, c.name, if c.is_ir { ", IR" } else { "" }),
        ),
        Some(c) if !c.accessible => Check::new(
            "Camera configured",
            Status::Warn,
            format!("{path} ({}) not openable right now", c.name),
        ),
        Some(c) => Check::new(
            "Camera configured",
            Status::Warn,
            format!("{path} ({}) is not a capture device", c.name),
        ),
        None => Check::new(
            "Camera configured",
            Status::Warn,
            format!("{path} not present (node numbering can change across reboots)"),
        ),
    }
}

fn check_models(user: &str) -> Check {
    match crate::model::list(user) {
        Ok(models) if !models.is_empty() => Check::new(
            "Face model enrolled",
            Status::Ok,
            format!("{} model(s) for {user}", models.len()),
        ),
        Ok(_) => Check::new(
            "Face model enrolled",
            Status::Warn,
            format!("no models for {user} — run `howdyctl add`"),
        ),
        Err(e) => Check::new("Face model enrolled", Status::Warn, e.to_string()),
    }
}
