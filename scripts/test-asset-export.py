#!/usr/bin/env python3
"""Execute target-profile exporters, then compile and compare their complete COM bundles.

Run outside Cargo: all child builds are sequential and share the selected target
root. Temporary package manifests reuse the workspace lock and exact published
COM versions. An optional work directory retains generated sources and logs.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
PROFILES = '''
[profile.release]
opt-level = "z"
panic = "abort"
lto = "fat"
codegen-units = 1
debug = 0
strip = "symbols"
[profile.min-size]
inherits = "release"
incremental = false
'''


def rust_path(path: Path) -> str:
    """Rust and TOML quoted path literals preserve spaces and native separators."""
    return json.dumps(str(path), ensure_ascii=False)


def package(directory: Path, name: str, extra: str, source: str) -> Path:
    """An isolated consumer owns no additional target directory or runtime layer."""
    (directory / "src").mkdir(parents=True, exist_ok=True)
    manifest = f'''[package]
name = "{name}"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
neocogi-gfx-support = {{ path = {rust_path(ROOT / 'crates/gfx/support')}, features = ["compiled-assets"] }}
neocogi-com = {{ version = "=0.1.0", features = ["std", "runtime"] }}
neocogi-com-host = "=0.1.0"
{extra}
{PROFILES}
'''
    (directory / "Cargo.toml").write_text(manifest)
    shutil.copyfile(ROOT / "Cargo.lock", directory / "Cargo.lock")
    (directory / "src/main.rs").write_text(source)
    return directory / "Cargo.toml"


def qualify(work: Path) -> None:
    """Expected bytes come from input records, independently of source emission."""
    work.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    target = Path(environment.get("CARGO_TARGET_DIR", str(ROOT / "target")))
    environment["CARGO_TARGET_DIR"] = str(target if target.is_absolute() else ROOT / target)
    fixture = ROOT / "crates/gfx/impl/tests/support/export_data.rs"
    generator_source = f'''#[path = {rust_path(fixture)}] mod fixture;
#[path = {rust_path(ROOT / 'examples/assets/src/recipe_data.rs')}] mod recipe;
use neocogi_gfx_impl::atlas::{{Builder, Config, FontAsset, IconAsset}};
fn main() {{
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let work = std::path::Path::new(&args[0]);
    let label = &args[1];
    let root = std::path::Path::new(&args[2]);
    let data = fixture::data();
    data.validate().unwrap();
    neocogi_gfx_impl::export::write_rust(&data, &work.join(format!("fixture-{{label}}.rs"))).unwrap();
    std::fs::write(work.join(format!("fixture-{{label}}.bin")), fixture::snapshot(&data)).unwrap();
    let config = Config {{ texture_width: recipe::SIZE.0 as usize, texture_height: recipe::SIZE.1 as usize,
        white_icon: root.join(recipe::WHITE),
        icons: recipe::ICONS.iter().map(|&(name, path)| IconAsset {{ name: name.into(), path: root.join(path) }}).collect(),
        fonts: recipe::FONTS.iter().map(|&(name, path, size)| FontAsset {{ name: name.into(), path: root.join(path), size: size as usize }}).collect(),
    }};
    let original = Builder::from_config(&config).unwrap().build().unwrap();
    std::fs::write(work.join(format!("default-{{label}}.bin")), fixture::snapshot(&original)).unwrap();
}}
'''
    generator = package(work / "generator", "neocogi-export-profile-fixture",
        f'neocogi-gfx-impl = {{ path = {rust_path(ROOT / "crates/gfx/impl")}, features = ["builder", "save-to-rust"] }}', generator_source)

    def run(label: str, arguments: list[str]) -> None:
        """Preserve exact command, output and exit without interleaving Cargo builds."""
        print(label, flush=True)
        with (work / f"{label}.log").open("w") as log:
            log.write(json.dumps(arguments) + "\n")
            log.flush()
            result = subprocess.run(arguments, cwd=ROOT, env=environment, stdout=log, stderr=subprocess.STDOUT)
        if result.returncode:
            sys.stderr.write((work / f"{label}.log").read_text())
            raise RuntimeError(f"{label} failed ({result.returncode}); see {work / (label + '.log')}")

    for label, profile in [("dev", []), ("min-size", ["--profile", "min-size"])]:
        run(f"generate-{label}", ["cargo", "run", "--offline", *profile, "--manifest-path", str(generator), "--", str(work), label, str(ROOT)])
        run(f"cli-{label}", ["cargo", "run", "--offline", *profile, "-p", "neocogi-asset-tools", "--bin", "neocogi-atlas-export", "--", "--output", str(work / f"default-{label}.rs")])
    for kind in ["fixture", "default"]:
        assert (work / f"{kind}-dev.bin").read_bytes() == (work / f"{kind}-min-size.bin").read_bytes(), f"{kind} input records differ across profiles"
    modules = "\n".join(f'mod {kind}_{label.replace("-", "_")} {{ include!({rust_path(work / (kind + "-" + label + ".rs"))}); }}' for kind in ["fixture", "default"] for label in ["dev", "min-size"])
    checks = []
    for kind in ["fixture", "default"]:
        for label in ["dev", "min-size"]:
            module = f'{kind}_{label.replace("-", "_")}'
            checks.append(f'''let data = {module}::bundle_data().unwrap();
    data.validate().unwrap();
    assert!(fixture::snapshot(&data) == include_bytes!({rust_path(work / (kind + '-dev.bin'))}).as_slice(), "{module}: emitted data differs from input");
    let bundle = {module}::load_bundle().unwrap();
    let copied = neocogi_gfx_support::AssetData::read(bundle.as_com_ref()).unwrap();
    assert!(fixture::snapshot(&copied) == fixture::snapshot(&data), "{module}: COM copy differs");''')
    source = f'''#[path = {rust_path(fixture)}] mod fixture;
{modules}
fn main() {{
    // One ordinary process runtime supplies the published COM implementations.
    let _runtime = unsafe {{ neocogi_com::Runtime::__from_host_api(neocogi_com_host::initialize_process()) }}.unwrap();
    {''.join(checks)}
    println!("all generated fields, bytes and COM copies agree");
}}
'''
    consumer = package(work / "consumer", "neocogi-export-profile-consumer", "", source)
    for label, profile in [("dev", []), ("min-size", ["--profile", "min-size"])]:
        run(f"consume-{label}", ["cargo", "run", "--offline", *profile, "--manifest-path", str(consumer)])
    print("Asset export round trip passed in dev and min-size.", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-dir", type=Path, help="retain sources and compiler logs at this path")
    args = parser.parse_args()
    if args.work_dir:
        qualify(args.work_dir.resolve())
    else:
        with tempfile.TemporaryDirectory(prefix="neocogi-export-roundtrip-") as temporary:
            qualify(Path(temporary))


if __name__ == "__main__":
    main()
