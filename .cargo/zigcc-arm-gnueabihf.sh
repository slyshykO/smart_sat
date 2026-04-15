#!/usr/bin/env bash
set -euo pipefail

find_zig() {
  local probe

  is_valid_zig() {
    probe="$1"
    [[ -x "$probe" ]] || return 1
    "$probe" version >/dev/null 2>&1
  }

  if [[ -n "${ZIG_BIN:-}" && -x "${ZIG_BIN}" ]]; then
    if is_valid_zig "${ZIG_BIN}"; then
      printf '%s\n' "${ZIG_BIN}"
      return 0
    fi
  fi

  if is_valid_zig "$HOME/bin/zig"; then
    printf '%s\n' "$HOME/bin/zig"
    return 0
  fi

  local candidate
  for candidate in "$HOME"/bin/zig*; do
    if is_valid_zig "$candidate"; then
      printf '%s\n' "$candidate"
      return 0
    fi

    if [[ -d "$candidate" ]] && is_valid_zig "$candidate/zig"; then
      printf '%s\n' "$candidate/zig"
      return 0
    fi
  done

  return 1
}

ZIG_PATH="$(find_zig || true)"
if [[ -z "$ZIG_PATH" ]]; then
  echo "error: zig not found. Set ZIG_BIN or place executable at ~/bin/zig*" >&2
  exit 127
fi

GLIBC_VERSION="${ZIG_GLIBC_VERSION:-2.28}"
TARGET_TRIPLE="arm-linux-gnueabihf.${GLIBC_VERSION}"

exec "$ZIG_PATH" cc -target "$TARGET_TRIPLE" "$@"