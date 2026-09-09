#!/usr/bin/env bash
# Build the real DOM listener tests, serve their wasm bindings, and assert the browser result.
set -euo pipefail
workspace_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$workspace_root"
suite="${1:-input}"
case "$suite" in
  input) package="neocogi-render-plugin"; fixture="crates/render/plugin/tests/browser-input.html" ;;
  full) package="neocogi-demo-full"; fixture="demos/full/tests/browser-full.html" ;;
  *) echo "usage: $0 [input|full]" >&2; exit 2 ;;
esac
site_dir="${CARGO_TARGET_DIR:-$workspace_root/target}/browser-$suite-tests"
build_log="$(mktemp)"
server_pid=""
trap 'if [[ -n "$server_pid" ]]; then kill "$server_pid" 2>/dev/null || true; fi; rm -f "$build_log"' EXIT
port="${NEOCOGI_BROWSER_TEST_PORT:-8118}"
wasm_bindgen="${NEOCOGI_WASM_BINDGEN:-$workspace_root/target/web-tools/bin/wasm-bindgen}"
if [[ ! -x "$wasm_bindgen" ]]; then
  echo "Run ./scripts/web-demo.sh build first to install the matching wasm-bindgen CLI." >&2
  exit 1
fi
browser="${CHROME_BIN:-}"
if [[ -z "$browser" ]]; then
  for candidate in google-chrome chromium chromium-browser; do
    if command -v "$candidate" >/dev/null; then browser="$candidate"; break; fi
  done
fi
if [[ -z "$browser" ]]; then echo "Set CHROME_BIN to a Chromium-compatible browser." >&2; exit 1; fi

# Cargo's artifact message avoids selecting a stale test executable left by another feature set.
cargo test -p "$package" --lib --no-run --no-default-features --features webgl \
  --target wasm32-unknown-unknown --message-format=json > "$build_log"
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
python3 -m http.server "$port" --bind 127.0.0.1 --directory "$site_dir" > "$site_dir/server.log" 2>&1 &
server_pid=$!
curl --retry-connrefused --retry 10 --retry-delay 1 --fail --silent "http://127.0.0.1:$port" > /dev/null
kill -0 "$server_pid" # Fail if another process already owned the requested port.
# This isolated fixture serves only repository tests. Disabling the browser sandbox supports
# container CI runners; the browser process exits after dumping the completed DOM. Piping stdout
# also supports Snap Chromium, whose launcher does not preserve a regular-file output descriptor.
"$browser" --headless --no-sandbox --enable-unsafe-swiftshader --use-gl=angle --use-angle=swiftshader --virtual-time-budget=15000 \
  --dump-dom "http://127.0.0.1:$port" | cat > "$site_dir/result.html"
if ! rg -q '<pre id="result">PASS</pre>' "$site_dir/result.html"; then
  cat "$site_dir/result.html" >&2
  exit 1
fi
echo "Browser $suite regression passed."
