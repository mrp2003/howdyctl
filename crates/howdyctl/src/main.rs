//! howdyctl — a TUI + CLI to manage Howdy face authentication.
use clap::{Parser, Subcommand};
use howdy::{camera, config::Config, doctor, model, test};

mod app;
mod elevate;
mod ui;

#[derive(Parser)]
#[command(
    name = "howdyctl",
    version,
    about = "Manage Howdy face authentication (TUI + CLI)"
)]
struct Cli {
    /// Launch the TUI with fake data (no Howdy / camera needed; for demos & screenshots).
    #[arg(long)]
    demo: bool,
    /// Operate on this user instead of the current one.
    #[arg(long, global = true)]
    user: Option<String>,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// List detected cameras (IR vs RGB, capture capability).
    List,
    /// List enrolled face models for the user.
    Models,
    /// Enroll a new face model (needs admin; pops a pkexec prompt).
    Add {
        /// Optional label for the model.
        label: Option<String>,
    },
    /// Remove a face model by id (needs admin).
    Remove {
        /// Model id from `howdyctl models`.
        id: u32,
    },
    /// Remove all of the user's face models (needs admin).
    Clear,
    /// Run a recognition test and print the result + match distance.
    Test,
    /// Run health checks on the Howdy install.
    Doctor,
    /// Point Howdy at a camera device, e.g. `set-camera /dev/video2` (needs admin).
    SetCamera {
        /// Path to a `/dev/video*` capture node.
        path: String,
    },
    /// Set the match certainty threshold, e.g. `certainty 4.0` (needs admin).
    Certainty {
        /// Threshold value (lower = stricter).
        value: f64,
    },
    /// Set the per-attempt timeout in seconds (needs admin).
    Timeout {
        /// Seconds to scan before giving up.
        secs: u32,
    },
    /// Set an arbitrary config.ini key (needs admin). Used by the TUI.
    #[command(hide = true)]
    SetConfig {
        /// Config key, e.g. `end_report`.
        key: String,
        /// New value.
        value: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let user = cli.user.clone().unwrap_or_else(howdy::current_user);

    match cli.cmd {
        None => app::run(cli.demo, user)?,

        Some(Cmd::List) => {
            let cams = camera::detect();
            if cams.is_empty() {
                println!("no /dev/video* devices found");
            }
            for c in cams {
                println!(
                    "{:<12} {:<22} {:<8} {}",
                    c.path.display(),
                    c.name,
                    if c.is_ir { "IR" } else { "RGB" },
                    if c.can_capture { "capture" } else { "metadata" },
                );
            }
        }

        Some(Cmd::Models) => {
            let models = model::list(&user)?;
            if models.is_empty() {
                println!("no face models enrolled for {user}");
            }
            for m in models {
                println!("{:<3} {}  {}", m.id, model::format_time(m.time), m.label);
            }
        }

        Some(Cmd::Test) => {
            let r = test::run(&user)?;
            println!("{}  (exit {})", r.message, r.exit_code);
            println!("threshold (certainty): {:.1}", r.threshold);
            match r.distance {
                Some(d) => println!(
                    "best match distance:   {:.1}  (margin {:+.1})",
                    d,
                    r.margin().unwrap_or(0.0)
                ),
                None => println!(
                    "best match distance:   unknown (enable debug.end_report to measure it)"
                ),
            }
            if let (Some(f), Some(fps)) = (r.frames, r.fps) {
                println!("frames scanned:        {f} ({fps:.1} fps)");
            }
        }

        Some(Cmd::Doctor) => {
            for c in doctor::run(&user) {
                let mark = match c.status {
                    doctor::Status::Ok => "[ ok ]",
                    doctor::Status::Warn => "[warn]",
                    doctor::Status::Fail => "[fail]",
                };
                println!("{mark} {:<26} {}", c.name, c.detail);
            }
        }

        // --- privileged actions: perform if root, else re-run under pkexec --------
        Some(Cmd::Add { label }) => priv_action(&both_args("add", label.clone()), || {
            model::add(&user, label.as_deref())
        })?,

        Some(Cmd::Remove { id }) => priv_action(&sub_args("remove", &[id.to_string()]), || {
            model::remove(&user, id)
        })?,

        Some(Cmd::Clear) => priv_action(&sub_args("clear", &[]), || model::clear(&user))?,

        Some(Cmd::SetCamera { path }) => {
            priv_action(&sub_args("set-camera", std::slice::from_ref(&path)), || {
                config_set("device_path", &path)
            })?
        }

        Some(Cmd::Certainty { value }) => {
            priv_action(&sub_args("certainty", &[format!("{value}")]), || {
                config_set("certainty", &format!("{value}"))
            })?
        }

        Some(Cmd::Timeout { secs }) => {
            priv_action(&sub_args("timeout", &[secs.to_string()]), || {
                config_set("timeout", &secs.to_string())
            })?
        }

        Some(Cmd::SetConfig { key, value }) => priv_action(
            &sub_args("set-config", &[key.clone(), value.clone()]),
            || config_set(&key, &value),
        )?,
    }
    Ok(())
}

/// Build the argv to re-run a subcommand with no positional args, carrying `--user`.
fn sub_args(sub: &str, positionals: &[String]) -> Vec<String> {
    let user = howdy::current_user();
    let mut v = vec!["--user".to_string(), user, sub.to_string()];
    v.extend_from_slice(positionals);
    v
}

/// Like [`sub_args`] but for a subcommand whose only positional is an optional label.
fn both_args(sub: &str, label: Option<String>) -> Vec<String> {
    let user = howdy::current_user();
    let mut v = vec!["--user".to_string(), user, sub.to_string()];
    if let Some(l) = label {
        v.push(l);
    }
    v
}

/// Perform `run` if we are root; otherwise re-execute ourselves under `pkexec` with
/// `args` and exit with that status.
fn priv_action<F>(args: &[String], run: F) -> anyhow::Result<()>
where
    F: FnOnce() -> anyhow::Result<()>,
{
    if elevate::is_root() {
        run()
    } else {
        let status = elevate::run(args)?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn config_set(key: &str, value: &str) -> anyhow::Result<()> {
    let mut cfg = Config::load()?;
    if !cfg.set(key, value) {
        anyhow::bail!("key '{key}' not found in config.ini");
    }
    cfg.save()?;
    Ok(())
}
