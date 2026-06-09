//! Run a one-shot recognition test via Howdy's `compare.py` and report the result.
//!
//! Runs as the normal user (no root). The exact match *distance* is only emitted by
//! `compare.py` when `debug.end_report = true`, so [`TestResult::distance`] is
//! `None` otherwise — `howdyctl` offers a toggle for that.
use std::process::Command;

use crate::Config;

/// Outcome of a single recognition attempt.
#[derive(Debug, Clone)]
pub struct TestResult {
    /// The face matched (exit code 0).
    pub matched: bool,
    /// `compare.py` exit code.
    pub exit_code: i32,
    /// Configured `certainty` threshold (a match needs distance ≤ this).
    pub threshold: f64,
    /// Best match distance achieved, in the same units as `threshold`
    /// (only known when detailed reporting is enabled).
    pub distance: Option<f64>,
    /// Frames scanned, if reported.
    pub frames: Option<u32>,
    /// Scan rate, if reported.
    pub fps: Option<f64>,
    /// Human-readable summary.
    pub message: String,
}

impl TestResult {
    /// How much margin the winning frame had under the threshold (positive = comfortable).
    pub fn margin(&self) -> Option<f64> {
        self.distance.map(|d| self.threshold - d)
    }
}

/// Run `compare.py <user>` once and interpret the result.
pub fn run(user: &str) -> anyhow::Result<TestResult> {
    let base = crate::base_dir();
    let threshold = Config::load()
        .ok()
        .and_then(|c| c.get_f64("certainty"))
        .unwrap_or(3.5);

    let output = Command::new("python3")
        .arg(base.join("compare.py"))
        .arg(user)
        .output()?;

    let exit_code = output.status.code().unwrap_or(-1);
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));

    let distance = parse_certainty(&text);
    let (frames, fps) = parse_frames(&text);

    let message = match exit_code {
        0 => "Face matched".to_string(),
        10 => "No face model enrolled for this user".to_string(),
        11 => "No match within the timeout".to_string(),
        12 => "Aborted".to_string(),
        13 => "Image too dark (check lighting / dark_threshold)".to_string(),
        c => format!("compare.py exited with code {c}"),
    };

    Ok(TestResult {
        matched: exit_code == 0,
        exit_code,
        threshold,
        distance,
        frames,
        fps,
        message,
    })
}

/// Parse `Certainty of winning frame: 2.900` → `2.9`.
fn parse_certainty(text: &str) -> Option<f64> {
    let line = text
        .lines()
        .find(|l| l.contains("Certainty of winning frame:"))?;
    line.rsplit(':').next()?.trim().parse().ok()
}

/// Parse `Frames searched: 120 (14.79 fps)` → `(120, 14.79)`.
fn parse_frames(text: &str) -> (Option<u32>, Option<f64>) {
    let Some(line) = text.lines().find(|l| l.contains("Frames searched:")) else {
        return (None, None);
    };
    let frames = line
        .split_whitespace()
        .find_map(|tok| tok.parse::<u32>().ok());
    let fps = line
        .split('(')
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse::<f64>().ok());
    (frames, fps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_winning_certainty() {
        let report =
            "Searching for known face\nCertainty of winning frame: 2.900\nWinning model: 0";
        assert_eq!(parse_certainty(report), Some(2.9));
        assert_eq!(parse_certainty("no report here"), None);
    }

    #[test]
    fn parses_frame_count_and_fps() {
        let report = "Frames searched: 120 (14.79 fps)\nBlack frames ignored: 0";
        assert_eq!(parse_frames(report), (Some(120), Some(14.79)));
    }

    #[test]
    fn margin_is_threshold_minus_distance() {
        let r = TestResult {
            matched: true,
            exit_code: 0,
            threshold: 4.0,
            distance: Some(2.9),
            frames: None,
            fps: None,
            message: String::new(),
        };
        assert!((r.margin().unwrap() - 1.1).abs() < 1e-9);
    }
}
