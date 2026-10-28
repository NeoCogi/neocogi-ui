#!/usr/bin/env python3
"""Exercise real Cargo outputs, cache restoration and relocated native startup.

This is an acceptance test, not an application build helper. Production staging
never invokes Cargo. --run requires an SDL/Vulkan-capable desktop (or Xvfb).
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
LIBRARIES = tuple(f"libnc_{name}.so" for name in ("ui", "ui_nodes", "render", "gfx", "skin"))


def build(profile, renderer="vulkan"):
    """Use Cargo's real compiler outputs; never calculate dependency paths."""
    result = subprocess.run(
        ["cargo", "build", "-p", "neocogi-demo-full", "--profile", profile,
         "--no-default-features", "--features", f"dynamic,{renderer}",
         "--message-format=json-render-diagnostics"], cwd=ROOT,
        capture_output=True, text=True, check=False,
    )
    assert result.returncode == 0, result.stderr
    messages = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
    artifacts = [message for message in messages if message.get("reason") == "compiler-artifact"]
    executable = next(Path(a["executable"]) for a in artifacts if a.get("executable")
                      and a["target"]["name"] == "neocogi-demo-full")
    return executable, artifacts


def digest(path):
    """Compare complete artifacts rather than timestamps or backend labels."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cache_checks(profile, executable):
    """Restore a cached backend and deleted outputs without cleaning Cargo."""
    root = executable.parent
    renderer = root / "lib/libnc_render.so"
    expected = digest(renderer)
    build(profile, "glow")
    assert digest(renderer) != expected, "renderer switch did not update the deployed DLL"
    build(profile)
    assert digest(renderer) == expected, "cached Vulkan build retained the Glow DLL"
    library = root / "lib/libnc_ui.so"
    asset = root / "assets/WHITE.png"
    originals = {path: digest(path) for path in (library, asset)}
    for path in originals:
        path.unlink()
    build(profile)
    for path, expected in originals.items():
        assert digest(path) == expected, f"deleted output was not restored: {path}"
    # Cargo records the compilation start time. Outputs restored during macro
    # expansion are newer and can require one following compile to settle its
    # timestamp dep-info. This must converge, rather than rebuild indefinitely.
    build(profile)
    _, artifacts = build(profile)
    assert all(a["fresh"] for a in artifacts), "unchanged layout unnecessarily rebuilt Cargo units"


def inspect(executable):
    """Reject private paths in deployed ELF bytes, including loader metadata."""
    libraries = [executable.parent / "lib" / name for name in LIBRARIES]
    assert all(path.is_file() for path in libraries), libraries
    private = {str(ROOT), str(Path.home()), str(executable.parent)}
    for name in ("CARGO_HOME", "RUSTUP_HOME"):
        if os.environ.get(name):
            private.add(os.environ[name])
    for binary in [executable, *libraries]:
        data = binary.read_bytes()
        for path in private:
            assert path.encode() not in data, f"private build path {path!r} in {binary}"
        dynamic = subprocess.check_output(["readelf", "-d", binary], text=True)
        assert all(name not in dynamic for name in ("libnc_com", "libneocogi_com_host")), f"COM services must be supplied by the host: {binary}"
        search = [line for line in dynamic.splitlines() if "RPATH" in line or "RUNPATH" in line]
        for line in search:
            paths = line.split("[", 1)[1].split("]", 1)[0].split(":")
            assert all(path in ("$ORIGIN/lib", "$ORIGIN") for path in paths), line
        for line in dynamic.splitlines():
            if "(NEEDED)" in line:
                assert "/" not in line.split("[", 1)[1].split("]", 1)[0], line
        print(f"{binary.name}: {binary.stat().st_size} bytes, private-path scan passed")


def relocate(executable):
    """Rename the copied layout; prove required files have no build-tree fallback."""
    with tempfile.TemporaryDirectory(prefix="neocogi relocated ") as directory:
        parent = Path(directory)
        before, after = parent / "before", parent / "renamed app"
        before.mkdir()
        shutil.copy2(executable, before / executable.name)
        (before / "lib").mkdir()
        for name in LIBRARIES:
            shutil.copy2(executable.parent / "lib" / name, before / "lib" / name)
        for name in ("assets", "themes"):
            shutil.copytree(executable.parent / name, before / name)
        before.rename(after)
        environment = {key: value for key, value in os.environ.items()
                       if key not in ("LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH", "DYLD_FALLBACK_LIBRARY_PATH")}
        environment["NEOCOGI_DEMO_FRAMES"] = "2"

        def run():
            """Execute outside the app directory with a finite frame count."""
            return subprocess.run([after / executable.name], cwd=parent, env=environment,
                                  capture_output=True, text=True, timeout=45)

        result = run()
        assert result.returncode == 0, result.stdout + result.stderr
        for relative in ("lib/libnc_ui.so", "assets/NORMAL.ttf"):
            required = after / relative
            saved = required.with_name(required.name + ".hidden")
            required.rename(saved)
            try:
                result = run()
                assert result.returncode != 0, f"missing {relative} silently used a build-machine fallback"
            finally:
                saved.rename(required)
        print("Renamed full app ran for two frames; missing DLL/font failed without fallback")


def main():
    """Select optional cache/desktop acceptance boundaries explicitly."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", default="min-size", choices=("release", "min-size"))
    parser.add_argument("--cache", action="store_true")
    parser.add_argument("--run", action="store_true")
    options = parser.parse_args()
    executable, _ = build(options.profile)
    if options.cache:
        cache_checks(options.profile, executable)
    inspect(executable)
    if options.run:
        relocate(executable)


if __name__ == "__main__":
    main()
