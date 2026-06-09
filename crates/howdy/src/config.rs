//! Read and (when root) write Howdy's `config.ini`.
//!
//! Howdy's keys are unique across the file, so we treat it as flat `key = value`
//! lines — the same line-oriented approach Howdy's own installer uses — preserving
//! comments, sections and indentation on write.
use std::fs;
use std::io;
use std::path::PathBuf;

/// An in-memory view of `config.ini`. Mutations stay in memory until [`Config::save`].
#[derive(Debug, Clone)]
pub struct Config {
    path: PathBuf,
    text: String,
}

impl Config {
    /// Load the live Howdy config from disk.
    pub fn load() -> io::Result<Config> {
        let path = crate::base_dir().join("config.ini");
        let text = fs::read_to_string(&path)?;
        Ok(Config { path, text })
    }

    /// Path of the underlying file.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Raw text (e.g. for display).
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Look up the first non-comment `key = value`, trimmed.
    pub fn get(&self, key: &str) -> Option<String> {
        for line in self.text.lines() {
            let l = line.trim_start();
            if l.starts_with('#') || l.starts_with(';') {
                continue;
            }
            if let Some((k, v)) = l.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
        None
    }

    /// [`Config::get`] parsed as `f64`.
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key)?.parse().ok()
    }

    /// [`Config::get`] parsed as `u32`.
    pub fn get_u32(&self, key: &str) -> Option<u32> {
        self.get(key)?.parse().ok()
    }

    /// [`Config::get`] parsed as a boolean (`true`/`false`, case-insensitive).
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key)?.to_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }

    /// Set `key` in memory, replacing the first non-comment occurrence and
    /// preserving its indentation. Returns `false` if the key was not present.
    pub fn set(&mut self, key: &str, value: &str) -> bool {
        let mut out = String::with_capacity(self.text.len() + value.len());
        let mut replaced = false;
        for line in self.text.lines() {
            let trimmed = line.trim_start();
            let is_comment = trimmed.starts_with('#') || trimmed.starts_with(';');
            if !replaced && !is_comment {
                if let Some((k, _)) = line.split_once('=') {
                    if k.trim() == key {
                        let indent = &line[..line.len() - trimmed.len()];
                        out.push_str(indent);
                        out.push_str(key);
                        out.push_str(" = ");
                        out.push_str(value);
                        out.push('\n');
                        replaced = true;
                        continue;
                    }
                }
            }
            out.push_str(line);
            out.push('\n');
        }
        self.text = out;
        replaced
    }

    /// Write the config back to disk (requires write access — i.e. root).
    pub fn save(&self) -> io::Result<()> {
        fs::write(&self.path, &self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    fn cfg(text: &str) -> Config {
        Config {
            path: "/tmp/none".into(),
            text: text.to_string(),
        }
    }

    #[test]
    fn reads_values_ignoring_comments() {
        let c = cfg("[video]\n# certainty = 9.9\ncertainty = 4.0\ndevice_path = /dev/video2\n");
        assert_eq!(c.get_f64("certainty"), Some(4.0));
        assert_eq!(c.get("device_path").as_deref(), Some("/dev/video2"));
        assert_eq!(c.get("missing"), None);
    }

    #[test]
    fn set_preserves_indentation_and_replaces_once() {
        let mut c = cfg("[video]\n\tcertainty = 2.8\ncertainty = 2.8\n");
        assert!(c.set("certainty", "4.0"));
        // only the first non-comment occurrence is rewritten
        assert_eq!(c.text(), "[video]\n\tcertainty = 4.0\ncertainty = 2.8\n");
    }

    #[test]
    fn set_reports_missing_key() {
        let mut c = cfg("certainty = 4.0\n");
        assert!(!c.set("timeout", "8"));
    }
}
