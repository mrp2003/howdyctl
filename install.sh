#!/usr/bin/env bash
# Build + install howdyctl into $PREFIX/bin.
#
# howdyctl does not need a udev rule: it reads your camera through the access your
# login session already has, and elevates the few root-only actions (enroll, config
# edits) with pkexec. So this just builds and installs the binary.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${PREFIX:-/usr/local}"
BIN="$PREFIX/bin/howdyctl"

echo "==> Building howdyctl (release)…"
cargo build --release --manifest-path "$REPO_DIR/Cargo.toml"

echo "==> Installing binary to $BIN (sudo)…"
sudo install -Dm0755 "$REPO_DIR/target/release/howdyctl" "$BIN"

if ! [ -f /lib/security/howdy/config.ini ] && ! [ -f /usr/lib/security/howdy/config.ini ]; then
  echo
  echo "Note: Howdy itself does not appear to be installed."
  echo "      howdyctl manages an existing Howdy install — see https://github.com/boltgolt/howdy"
fi

cat <<EOF

Done. ✅
  • Binary: $BIN

Get started:

  howdyctl            # launch the TUI
  howdyctl doctor     # check your Howdy install
  howdyctl --demo     # try the TUI with no hardware

EOF
