#!/usr/bin/env bash
# Exercise native Cargo artifacts and the real full application, without staging another build graph.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
backend="${1:-glow}"
case "$backend" in glow|vulkan|wgpu) ;; *) echo "usage: $0 [glow|vulkan|wgpu]" >&2; exit 2 ;; esac
features="dynamic,$backend"
if [[ "${NEOCOGI_TEST_BUILD_ONLY:-0}" == 1 ]]; then
    exec cargo build -p neocogi-demo-full --no-default-features --features "$features"
fi
NEOCOGI_DEMO_FRAMES=3 cargo run -p neocogi-demo-full --no-default-features --features "$features"
# SDL permits one event pump per process; native fixtures own it in turn.
cargo test -p neocogi-demo-full --lib --no-default-features --features "$features" \
    --config "target.'cfg(any(unix, windows))'.runner = ['python3', '$PWD/scripts/run-native-test.py']" \
    -- --include-ignored --test-threads=1
# Exercise shader inputs and actual pixels as well as the application's COM/UI integration.
cargo test -p neocogi-shader-compiler -p neocogi-render-gpu --lib
cargo test -p neocogi-render-plugin --lib --test compiler_activation --no-default-features --features "$backend" \
    -- --include-ignored --test-threads=1
