# howdyctl — Roadmap / TODO

Future implementations and ideas for [howdyctl](https://github.com/mrp2003/howdyctl).
Crates: `howdyctl` (binary) + `howdy` (library).

Status today (v0.1.0):
- ratatui TUI with Cameras / Models / Test / Doctor tabs
- Camera detection via v4l2 `QUERYCAP` (IR vs RGB, capture vs metadata)
- Model list (reads the model JSON directly — no root) + enroll / delete via pkexec
- Recognition Test view with a threshold gauge + match-distance margin
- Doctor: dlib/opencv import, data files, `pam.py` py3-compat, dir traversal, PAM
  wiring, camera, enrolled models — plus `doctor --fix` to auto-repair
- CLI for every action (`list`, `models`, `add`, `remove`, `clear`, `test`,
  `doctor`, `set-camera`, `certainty`, `timeout`)
- `--demo` mode, install script, CI (fmt/clippy/test/build)

---

## 1. The Test / tuner view (the differentiator)
- [ ] **Continuous live gauge**: stream the per-frame match distance in real time
      (own recognition loop via the `dlib` crate, or a small helper that prints each
      frame's distance) instead of one-shot `compare.py`.
- [ ] Live IR camera **preview** in the terminal (sixel/kitty graphics, or ASCII).
- [ ] "Suggest a threshold" — sample N frames, recommend a certainty with margin.
- [ ] Non-blocking test (spinner + cancel) so the UI doesn't freeze during a scan.

## 2. Models
- [x] **Inline label entry** when enrolling from the TUI.
- [ ] Per-model **test** (which model matched, and at what distance).
- [ ] Rename models; show enrollment thumbnail if snapshots are on.
- [ ] Clear-all with a confirm step in the TUI.

## 3. Config
- [ ] Editable **timeout / dark_threshold / use_cnn** from the TUI (sliders).
- [ ] Toggle `detection_notice`, `no_confirmation`, `ignore_ssh`, lid handling.
- [ ] Back up `config.ini` before writes; one-key restore.
- [ ] Surface and edit the `[snapshots]` settings (and a snapshots viewer).

## 4. Doctor & repair
- [x] **`howdyctl doctor --fix`**: apply the obvious repairs (chmod dirs traversable,
      patch the `pam.py` Python-2 import, run `pam-auth-update`, fix `device_path`,
      re-download model data). Idempotent.
- [ ] Detect the orphaned-camera-lock situation and offer to free it.
- [ ] Check `libpam-python` / `pam_python.so` presence and Python ABI match.
- [ ] GDM/greeter login-screen wiring check.

## 5. Library (`howdy` crate)
- [ ] Parse the v4l2 format list (confirm a node really streams `GREY`/`YUYV`).
- [ ] Read `LampArrayAttributes`-style richer camera info (resolutions, formats).
- [ ] A typed `Config` with known keys instead of stringly-typed get/set.
- [ ] Optional `dlib` feature for in-process recognition (powers the live gauge).
- [ ] Decide on a non-colliding crates.io name before publishing the lib.

## 6. Packaging & distribution
- [ ] Publish `howdyctl` (and the lib) to crates.io.
- [ ] **AUR** package, `.deb`, Nix flake, Homebrew tap.
- [ ] `cargo-dist` for prebuilt release binaries; `cargo-binstall` support.
- [ ] Optional polkit policy so the pkexec prompt has a friendly action name.
- [ ] man page (`howdyctl.1`).

## 7. Quality & docs
- [ ] Integration tests against a fake Howdy tree (temp dir) — no real install needed.
- [ ] Per-crate README for `howdy` + docs.rs polish.
- [ ] `CONTRIBUTING.md`, issue/PR templates, a "good first issue".
- [ ] Compatibility notes: distros / Howdy 2.x vs 3.x.

---

## Quick wins (do these first)
1. Editable timeout/dark_threshold sliders in the Test/Config view
2. Non-blocking test with a spinner
3. Publish to crates.io + an AUR package
