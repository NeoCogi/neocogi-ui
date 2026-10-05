#!/usr/bin/env bash
# Run the real full application tests against independently built, descriptor-validated native plugins.
set -euo pipefail
workspace_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$workspace_root"
backend="${1:-glow}"
case "$backend" in glow|vulkan|wgpu) ;; *) echo "usage: $0 [glow|vulkan|wgpu]" >&2; exit 2 ;; esac
target_dir="${CARGO_TARGET_DIR:-$workspace_root/target}"
if [[ "$target_dir" != /* ]]; then target_dir="$workspace_root/$target_dir"; fi
build_output="$(cargo run -p cargo-neocogi -- build --assembly full --variant "$backend")"
printf '%s\n' "$build_output"
package_dir="$(printf '%s\n' "$build_output" | sed -n 's/^Staged //p' | tail -n 1)"
if [[ -z "$package_dir" ]]; then echo "cargo-neocogi did not report an assembly directory" >&2; exit 1; fi
export NC_COM_RUNTIME_LINK_DIR="$package_dir"
case "$(uname -s)" in
  Linux) export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-rpath,\$ORIGIN" ;;
  Darwin) export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-rpath,@executable_path" ;;
esac
build_log="$(mktemp)"
trap 'rm -f "$build_log"' EXIT
cargo test -p neocogi-demo-full --lib --no-run --no-default-features --features "$backend" --message-format=json > "$build_log"
artifact="$(python3 - "$build_log" <<'PY'
import json, sys
for line in open(sys.argv[1]):
    message = json.loads(line)
    if message.get('reason') == 'compiler-artifact' and message.get('executable'):
        print(message['executable'])
PY
)"
# The fresh package is already independent of Cargo's build directories. Copy only the test runner
# into it and launch from /tmp so neither source-tree assets nor working-directory lookup can help.
test_dir="$target_dir/neocogi-tests/$backend"
rm -rf "$test_dir"
mkdir -p "$test_dir"
cp -R "$package_dir"/. "$test_dir/"
test_binary="$(basename -- "$artifact")"
cp "$artifact" "$test_dir/$test_binary"
echo "Staged application tests: $test_dir/$test_binary"
if [[ "${NEOCOGI_TEST_BUILD_ONLY:-0}" == 1 ]]; then exit 0; fi
cd "${TMPDIR:-/tmp}"
# SDL permits one event pump per process. Native window fixtures must own it in turn.
"$test_dir/$test_binary" --include-ignored --test-threads=1
