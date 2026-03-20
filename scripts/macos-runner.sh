#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <binary> [args...]" >&2
  exit 2
fi

binary="$1"
shift

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
entitlements="$repo_root/entitlements/capsa.entitlements"

if [[ "$(uname -s)" == "Darwin" && -f "$entitlements" ]]; then
  codesign --force --sign - --entitlements "$entitlements" "$binary" >/dev/null 2>&1 || {
    echo "warning: failed to codesign $binary with hypervisor entitlements" >&2
  }

  # Also sign sibling capsa-vmm binary when present, since capsa spawns it.
  vmm_binary="$(dirname "$binary")/capsa-vmm"
  if [[ -x "$vmm_binary" ]]; then
    codesign --force --sign - --entitlements "$entitlements" "$vmm_binary" >/dev/null 2>&1 || {
      echo "warning: failed to codesign $vmm_binary with hypervisor entitlements" >&2
    }
  fi
fi

exec "$binary" "$@"
