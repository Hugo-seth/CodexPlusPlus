#!/usr/bin/env bash
# Sync the project version across all manifests.
#
# Single source of truth: workspace.package.version in the root Cargo.toml.
# This script propagates that value to:
#   - apps/codex-plus-manager/package.json
#   - apps/codex-plus-manager/src-tauri/tauri.conf.json
#
# Usage:
#   scripts/sync-version.sh            # read version from Cargo.toml, write to others
#   scripts/sync-version.sh <version>  # set Cargo.toml to <version>, then propagate
#   scripts/sync-version.sh --check    # exit non-zero if any file is out of sync
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_TOML="$ROOT/Cargo.toml"
PACKAGE_JSON="$ROOT/apps/codex-plus-manager/package.json"
TAURI_CONF="$ROOT/apps/codex-plus-manager/src-tauri/tauri.conf.json"

read_cargo_version() {
  python3 - "$CARGO_TOML" <<'PY'
import re, sys
text = open(sys.argv[1], "r", encoding="utf-8").read()
match = re.search(
    r'\[workspace\.package\][^\[]*?\nversion\s*=\s*"([^"]+)"',
    text,
    flags=re.DOTALL,
)
print(match.group(1) if match else "")
PY
}

read_json_version() {
  python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['version'])" "$1"
}

write_json_version() {
  local file="$1" version="$2"
  python3 - "$file" "$version" <<'PY'
import json, sys
path, version = sys.argv[1], sys.argv[2]
with open(path, "r", encoding="utf-8") as fh:
    data = json.load(fh)
data["version"] = version
with open(path, "w", encoding="utf-8") as fh:
    json.dump(data, fh, indent=2, ensure_ascii=False)
    fh.write("\n")
PY
}

write_cargo_version() {
  local version="$1"
  python3 - "$CARGO_TOML" "$version" <<'PY'
import re, sys
path, version = sys.argv[1], sys.argv[2]
text = open(path, "r", encoding="utf-8").read()
text = re.sub(
    r'(\[workspace\.package\][^\[]*?\nversion\s*=\s*")[^"]+(")',
    r'\g<1>' + version + r'\g<2>',
    text,
    count=1,
    flags=re.DOTALL,
)
open(path, "w", encoding="utf-8").write(text)
PY
}

mode="sync"
target_version=""
if [[ "${1:-}" == "--check" ]]; then
  mode="check"
elif [[ -n "${1:-}" ]]; then
  mode="set"
  target_version="$1"
fi

case "$mode" in
  set)
    write_cargo_version "$target_version"
    version="$target_version"
    ;;
  *)
    version="$(read_cargo_version)"
    ;;
esac

if [[ -z "$version" ]]; then
  echo "ERROR: failed to read version from $CARGO_TOML" >&2
  exit 1
fi

pkg_version="$(read_json_version "$PACKAGE_JSON")"
tauri_version="$(read_json_version "$TAURI_CONF")"

if [[ "$mode" == "check" ]]; then
  drift=0
  if [[ "$pkg_version" != "$version" ]]; then
    echo "DRIFT: $PACKAGE_JSON has $pkg_version (expected $version)"
    drift=1
  fi
  if [[ "$tauri_version" != "$version" ]]; then
    echo "DRIFT: $TAURI_CONF has $tauri_version (expected $version)"
    drift=1
  fi
  if [[ "$drift" -eq 0 ]]; then
    echo "OK: all manifests at $version"
  fi
  exit "$drift"
fi

if [[ "$pkg_version" != "$version" ]]; then
  write_json_version "$PACKAGE_JSON" "$version"
  echo "updated $PACKAGE_JSON -> $version"
fi
if [[ "$tauri_version" != "$version" ]]; then
  write_json_version "$TAURI_CONF" "$version"
  echo "updated $TAURI_CONF -> $version"
fi
echo "version: $version"
