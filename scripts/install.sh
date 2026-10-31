#!/usr/bin/env bash
# Agent Arena installer. Safe to run more than once.
# Installs Rust if missing, builds everything, and puts the binaries in ./bin.
set -euo pipefail
cd "$(dirname "$0")/.."

say() { printf '\033[1;36m[agent-arena]\033[0m %s\n' "$*"; }

if ! command -v cargo >/dev/null 2>&1; then
  if [ -f "$HOME/.cargo/env" ]; then . "$HOME/.cargo/env"; fi
fi
if ! command -v cargo >/dev/null 2>&1; then
  say "Rust not found. Installing it with rustup (user-level, no sudo)..."
  command -v curl >/dev/null 2>&1 || { echo "curl is required to install Rust. Install curl, or Rust from https://rustup.rs, then re-run."; exit 1; }
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi
say "Using $(cargo --version)"

if command -v rustup >/dev/null 2>&1; then
  rustup target add wasm32-unknown-unknown >/dev/null
else
  say "rustup not found: make sure the wasm32-unknown-unknown target is installed for your Rust."
fi

say "Building the viewer (WASM)..."
cargo build -p lab-web --target wasm32-unknown-unknown --release
T="${CARGO_TARGET_DIR:-target}"
cp "$T/wasm32-unknown-unknown/release/lab_web.wasm" web/lab_web.wasm

say "Building the server and labctl..."
cargo build -p lab-server -p labctl --release
mkdir -p bin
cp "$T/release/lab-server" "$T/release/labctl" bin/

say "Installed. Binaries: $(pwd)/bin/lab-server and $(pwd)/bin/labctl"
say "Next: bash scripts/start.sh            (add --demo to see sample agents)"
echo "AGENT_ARENA_INSTALLED=$(pwd)"
