#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${root}"
shellcheck scripts/*.sh profiles/live/profiledef.sh profiles/live/airootfs/usr/local/bin/*
cargo fmt --all --check
cargo clippy -p aegis-common --all-targets -- -D warnings
cargo test -p aegis-common
