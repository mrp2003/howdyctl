//! Enumerate `/dev/video*` capture devices and label them IR vs RGB.
use std::fs;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use crate::ioctl;

/// A video4linux device node discovered on the system.
#[derive(Debug, Clone)]
pub struct Camera {
    /// Path to the node, e.g. `/dev/video2`.
    pub path: PathBuf,
    /// `videoN` index.
    pub index: u32,
    /// Human-readable name from sysfs, e.g. `ASUS IR camera`.
    pub name: String,
    /// Looks like an infrared camera (best for Howdy — works in the dark, harder to spoof).
    pub is_ir: bool,
    /// The node actually does video capture (vs. a metadata-only companion node).
    pub can_capture: bool,
    /// We were able to open the node (permissions / not busy).
    pub accessible: bool,
}

/// Detect all video4linux devices, sorted by index.
///
/// Capture-capability is probed with `VIDIOC_QUERYCAP`; IR-vs-RGB is inferred from
/// the device name. Never fails — a system with no cameras yields an empty list.
pub fn detect() -> Vec<Camera> {
    let mut cams = Vec::new();
    let dir = match fs::read_dir("/sys/class/video4linux") {
        Ok(d) => d,
        Err(_) => return cams,
    };

    let mut nodes: Vec<(u32, String)> = Vec::new();
    for entry in dir.flatten() {
        let fname = entry.file_name();
        let fname = fname.to_string_lossy();
        if let Some(idx) = fname
            .strip_prefix("video")
            .and_then(|n| n.parse::<u32>().ok())
        {
            let name = fs::read_to_string(entry.path().join("name"))
                .unwrap_or_default()
                .trim()
                .to_string();
            nodes.push((idx, name));
        }
    }
    nodes.sort_by_key(|(idx, _)| *idx);

    for (index, name) in nodes {
        let path = PathBuf::from(format!("/dev/video{index}"));
        let (can_capture, accessible) = probe(&path);
        cams.push(Camera {
            is_ir: name_is_ir(&name),
            path,
            index,
            name,
            can_capture,
            accessible,
        });
    }
    cams
}

/// The cameras that are usable as a Howdy `device_path`: real capture nodes.
/// IR cameras come first, then by index.
pub fn capture_devices() -> Vec<Camera> {
    let mut v: Vec<Camera> = detect().into_iter().filter(|c| c.can_capture).collect();
    v.sort_by(|a, b| b.is_ir.cmp(&a.is_ir).then(a.index.cmp(&b.index)));
    v
}

/// `(can_capture, accessible)` — opens the node read/write and asks the kernel.
fn probe(path: &Path) -> (bool, bool) {
    let file = match fs::OpenOptions::new().read(true).write(true).open(path) {
        Ok(f) => f,
        Err(_) => return (false, false),
    };
    match ioctl::querycap(file.as_raw_fd()) {
        Ok(cap) => {
            let effective = if cap.capabilities & ioctl::CAP_DEVICE_CAPS != 0 {
                cap.device_caps
            } else {
                cap.capabilities
            };
            (effective & ioctl::CAP_VIDEO_CAPTURE != 0, true)
        }
        // Opened but the query failed: accessible, capability unknown — assume no.
        Err(_) => (false, true),
    }
}

/// Heuristic: does this device name denote an infrared camera?
fn name_is_ir(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("infrared")
        || lower
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| word == "ir")
}

#[cfg(test)]
mod tests {
    use super::name_is_ir;

    #[test]
    fn ir_detection() {
        assert!(name_is_ir("ASUS IR camera"));
        assert!(name_is_ir("Integrated Infrared Camera"));
        assert!(name_is_ir("Chicony IR Camera"));
        assert!(!name_is_ir("ASUS FHD webcam"));
        assert!(!name_is_ir("Integrated RGB Camera"));
        // must match the whole word, not a substring like "third"
        assert!(!name_is_ir("third party cam"));
    }
}
