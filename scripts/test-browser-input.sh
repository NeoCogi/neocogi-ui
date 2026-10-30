#!/usr/bin/env bash
# Build the real DOM listener tests, serve their wasm bindings, and assert the browser result.
set -euo pipefail
workspace_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$workspace_root"
suite="${1:-input}"
case "$suite" in
  input) package="neocogi-render-plugin"; fixture="crates/render/plugin/tests/browser-input.html" ;;
  full) package="neocogi-demo-full"; fixture="examples/full/tests/browser-full.html" ;;
  *) echo "usage: $0 [input|full]" >&2; exit 2 ;;
esac
# Keep this small path policy in agreement with web-demo.sh; subprocess tests
# exercise both scripts together, including roots and executable paths with spaces.
target_dir="${CARGO_TARGET_DIR:-$workspace_root/target}"
if [[ "$target_dir" != /* ]]; then target_dir="$workspace_root/$target_dir"; fi
site_dir="$target_dir/browser-$suite-tests"
# Build diagnostics and the readiness signal belong to this invocation, even when
# another suite uses the same target directory or port.
session_dir="$(mktemp -d)"
build_log="$session_dir/build.json"
ready_file="$session_dir/server-ready"
server_pid=""
# Reap our child before removing its private publication directory. A failed bind
# must never cause cleanup to signal the unrelated process that owns the port.
cleanup() {
  if [[ -n "$server_pid" ]]; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  rm -rf "$session_dir"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
port="${NEOCOGI_BROWSER_TEST_PORT:-8118}"
wasm_bindgen="${NEOCOGI_WASM_BINDGEN:-$target_dir/web-tools/bin/wasm-bindgen}"
if [[ ! -x "$wasm_bindgen" ]]; then
  echo "Matching wasm-bindgen CLI not found at $wasm_bindgen. Run ./scripts/web-demo.sh build first to install it." >&2
  exit 1
fi
browser="${CHROME_BIN:-}"
if [[ -z "$browser" ]]; then
  for candidate in google-chrome chromium chromium-browser; do
    if command -v "$candidate" >/dev/null; then browser="$candidate"; break; fi
  done
fi
if [[ -z "$browser" ]]; then echo "Set CHROME_BIN to a Chromium-compatible browser." >&2; exit 1; fi

# Keep artifact selection machine-readable while rendering compiler errors to stderr;
# the temporary JSON stream is removed on exit, including failed compilations.
cargo test -p "$package" --lib --no-run --no-default-features --features webgl \
  --target wasm32-unknown-unknown --message-format=json-render-diagnostics > "$build_log"
artifact="$(python3 - "$build_log" <<'PY'
import json, sys
for line in open(sys.argv[1]):
    message = json.loads(line)
    if message.get('reason') == 'compiler-artifact' and message.get('executable'):
        print(message['executable'])
PY
)"
mkdir -p "$site_dir"
"$wasm_bindgen" --target web --out-dir "$site_dir" --out-name input-test "$artifact"
cp "$fixture" "$site_dir/index.html"
python3 - "$site_dir" "$port" "$ready_file" > "$site_dir/server.log" 2>&1 <<'PY' &
import functools
import http.server
from pathlib import Path
import sys

site, port, ready = sys.argv[1:]
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=site)
# Construction owns bind/listen. Only this child can publish readiness, after it
# has acquired the selected endpoint; another server's response is not evidence.
with http.server.ThreadingHTTPServer(('127.0.0.1', int(port)), handler) as server:
    Path(ready).write_text('ready\n')
    server.serve_forever()
PY
server_pid=$!
# Bound startup to ten seconds and retain the child's diagnostics on failure.
for ((attempt = 0; attempt < 100; attempt++)); do
  if [[ -f "$ready_file" ]] || ! kill -0 "$server_pid" 2>/dev/null; then break; fi
  sleep 0.1
done
if [[ ! -f "$ready_file" ]] || ! kill -0 "$server_pid" 2>/dev/null; then
  echo "Browser $suite server did not become ready on 127.0.0.1:$port." >&2
  cat "$site_dir/server.log" >&2
  exit 1
fi
# This isolated fixture serves only repository tests. Disabling the browser sandbox supports
# container CI runners; the browser process exits after dumping the completed DOM. Piping stdout
# also supports Snap Chromium, whose launcher does not preserve a regular-file output descriptor.
"$browser" --headless --no-sandbox --enable-unsafe-swiftshader --use-gl=angle --use-angle=swiftshader --virtual-time-budget=15000 \
  --force-device-scale-factor="${NEOCOGI_BROWSER_SCALE:-1}" --dump-dom "http://127.0.0.1:$port" | cat > "$site_dir/result.html"
# Like the package builder, the browser runner depends only on standard shell text utilities.
if ! grep -Fq '<pre id="result">PASS</pre>' "$site_dir/result.html"; then
  cat "$site_dir/result.html" >&2
  exit 1
fi
echo "Browser $suite regression passed."
