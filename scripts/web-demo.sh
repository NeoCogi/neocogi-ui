#!/usr/bin/env bash
set -euo pipefail

workspace_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
target="wasm32-unknown-unknown"
artifact_name="neocogi-demo-full"
target_dir="${CARGO_TARGET_DIR:-$workspace_root/target}"
if [[ "$target_dir" != /* ]]; then target_dir="$workspace_root/$target_dir"; fi
profile="${NEOCOGI_WEB_PROFILE:-release}"
artifact_dir="$target_dir/$target/$profile"
site_dir="$target_dir/web-demo"
staging_dir="$target_dir/web-demo-staging"
web_tools_dir="$target_dir/web-tools"
wasm_bindgen="${NEOCOGI_WASM_BINDGEN:-$web_tools_dir/bin/wasm-bindgen}"
wasm_bindgen_version="0.2.127"
command="${1:-serve}"
port="${2:-8000}"

build() {
  cd "$workspace_root"
  if [[ "$profile" != release && "$profile" != debug ]]; then
    echo "NEOCOGI_WEB_PROFILE must be release or debug" >&2; exit 2
  fi
  if ! rustup target list --installed | rg -q "^$target$"; then rustup target add "$target"; fi
  profile_args=()
  if [[ "$profile" == release ]]; then profile_args+=(--release); fi
  if [[ ! -x "$wasm_bindgen" ]] || [[ "$($wasm_bindgen --version)" != "wasm-bindgen $wasm_bindgen_version" ]]; then
    cargo install \
      --root "$web_tools_dir" \
      --version "$wasm_bindgen_version" \
      --locked \
      wasm-bindgen-cli
  fi
  cargo build \
    --manifest-path "$workspace_root/Cargo.toml" \
    -p neocogi-demo-full \
    --bin "$artifact_name" \
    --target "$target" \
    "${profile_args[@]}" \
    --no-default-features \
    --features webgl

  rm -rf "$staging_dir"
  mkdir -p "$staging_dir"
  "$wasm_bindgen" \
    --target web \
    --no-typescript \
    --out-dir "$staging_dir" \
    --out-name "$artifact_name" \
    "$artifact_dir/$artifact_name.wasm"
  install -m 0644 "$workspace_root/demos/full/web/index.html" "$staging_dir/index.html"
  touch "$staging_dir/.nojekyll"
  rm -rf "$site_dir"
  mv "$staging_dir" "$site_dir"

  echo "Web demo built in $site_dir"
}

case "$command" in
  build)
    build
    ;;
  serve)
    build
    echo "Serving http://127.0.0.1:$port"
    python3 -m http.server "$port" --bind 127.0.0.1 --directory "$site_dir"
    ;;
  *)
    echo "usage: $0 [build|serve] [port]" >&2
    exit 2
    ;;
esac
