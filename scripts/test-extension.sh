#!/usr/bin/env bash
# The host calls an extension built as an independent Cargo cdylib artifact.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
backend="${1:-glow}"
case "$backend" in glow|vulkan|wgpu) ;; *) echo "usage: $0 [glow|vulkan|wgpu]" >&2; exit 2 ;; esac
if [[ "${NEOCOGI_TEST_BUILD_ONLY:-0}" == 1 ]]; then
    exec cargo build -p neocogi-demo-smoke --no-default-features --features "dynamic-extension,$backend"
fi
exec cargo run -p neocogi-demo-smoke --no-default-features --features "dynamic-extension,$backend" -- --smoke
