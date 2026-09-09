#!/usr/bin/env bash
# Compile assemblies separately, then test loading and rendering without the source directory as cwd.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
backend=${1:-glow}
case "$backend" in glow|vulkan|wgpu) ;; *) echo "usage: $0 [glow|vulkan|wgpu]" >&2; exit 2 ;; esac
build_output="$(cargo run -p cargo-neocogi -- build --assembly smoke --variant "$backend" --variant extension)"
printf '%s\n' "$build_output"
package_dir="$(printf '%s\n' "$build_output" | sed -n 's/^Staged //p' | tail -n 1)"
if [[ -z "$package_dir" ]]; then echo "cargo-neocogi did not report an assembly directory" >&2; exit 1; fi
executable="$package_dir/neocogi-demo-smoke"
if [[ "${NEOCOGI_TEST_BUILD_ONLY:-0}" == 1 ]]; then
    printf '%s\n' "$executable"
    exit 0
fi
cd /tmp
exec "$executable" --smoke
