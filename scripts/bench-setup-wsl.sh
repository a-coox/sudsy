#!/usr/bin/env bash
# One-time setup for scripts/bench.sh tier 1: rustup (stable + wasm32-wasip2) and valgrind in WSL
# (Ubuntu), or on a Linux host. Run it from Git Bash or a Linux shell; apt asks for your sudo
# password. Safe to run again.
set -euo pipefail
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    wslpath="/mnt$(cd "$(dirname "$0")" && pwd)"  # Git Bash pwd is /c/...; WSL mounts it at /mnt/c/...
    MSYS_NO_PATHCONV=1 exec wsl -d "${BENCH_WSL_DISTRO:-Ubuntu}" -e bash "$wslpath/bench-setup-wsl.sh" ;;
esac
if ! command -v valgrind >/dev/null || ! command -v gcc >/dev/null || ! command -v curl >/dev/null; then
  sudo apt-get update
  sudo apt-get install -y valgrind build-essential curl
fi
if ! [ -x "$HOME/.cargo/bin/rustup" ]; then
  curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
fi
. "$HOME/.cargo/env"
rustup update stable
rustup target add wasm32-wasip2
echo "bench setup done: $(valgrind --version), $(rustc -V)"
