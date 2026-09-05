#!/usr/bin/env bash
# Compile assemblies separately, then test loading and rendering without the source directory as cwd.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
backend=${1:-glow}
cargo run -p microui-redux-tools --bin com-build -- --demo extension --backend "$backend"
target=${CARGO_TARGET_DIR:-"$root/target"}
if [[ "$target" != /* ]]; then target="$root/$target"; fi
executable="$target/com-native/debug/$backend/prebuilt/extension/microui-demo-com"
if [[ "${MICROUI_TEST_BUILD_ONLY:-0}" == 1 ]]; then
    printf '%s\n' "$executable"
    exit 0
fi
cd /tmp
exec "$executable" --smoke
