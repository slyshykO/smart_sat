#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-arm-unknown-linux-musleabihf}"
shift || true

case "$TARGET" in
  arm-unknown-linux-musleabihf|arm-unknown-linux-gnueabihf)
    ;;
  *)
    echo "error: unsupported target '$TARGET'" >&2
    echo "supported: arm-unknown-linux-musleabihf, arm-unknown-linux-gnueabihf" >&2
    exit 2
    ;;
esac

rustup target add "$TARGET"

if [[ "$TARGET" == "arm-unknown-linux-musleabihf" ]]; then
  if [[ -n "${RUSTFLAGS:-}" ]]; then
    export RUSTFLAGS="${RUSTFLAGS} -C target-feature=-crt-static"
  else
    export RUSTFLAGS="-C target-feature=-crt-static"
  fi
fi

exec cargo build --target "$TARGET" "$@"