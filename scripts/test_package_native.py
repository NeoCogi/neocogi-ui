#!/usr/bin/env python3
"""Exact-distribution acceptance using real applications and production staging macros.

The default suite checks rejection rules without Cargo. --production adds sequential
composition/cache checks and deliberately synchronized concurrent Cargo selections.
All candidates/evidence live outside Cargo's cache; authored inputs are never edited.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("package_native", ROOT / "scripts/package-native.py")
packager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packager)


def digest(path):
    """Compare immutable artifact contents rather than cache timestamps."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Admission(unittest.TestCase):
    """Malformed file sets and publication conflicts fail before replacing anything."""
    def test_paths(self):
        for value in ("", "../x", "/x", "x/../y", "C:/x", "x\\y"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                packager.relative_resource(value)
        self.assertEqual(packager.relative_resource("themes/a/font.ttf"), "themes/a/font.ttf")

    def test_exact_membership_and_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            app = Path(directory)
            (app / "extra").write_text("stale")
            with self.assertRaises(ValueError):
                packager.validate_candidate(app, "app", {"app"}, set())
            (app / "extra").unlink()
            if os.name == "posix":
                (app / "alias").symlink_to("missing")
                with self.assertRaises(ValueError):
                    packager.validate_candidate(app, "app", {"app"}, set())

    def test_only_root_staging_metadata_is_discarded(self):
        with tempfile.TemporaryDirectory() as directory:
            app = Path(directory)
            expected = {"app", "assets/.neocogi-layout.json"}
            for name in expected | {".neocogi-layout.json", ".neocogi-layout.lock", ".neocogi-unexpected"}:
                path = app / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(name)
            packager.discard_staging_metadata(app, expected)
            self.assertEqual(files(app), expected | {".neocogi-unexpected"})
            self.assertEqual((app / "assets/.neocogi-layout.json").read_text(), "assets/.neocogi-layout.json")
            with self.assertRaisesRegex(ValueError, "unexpected=.*neocogi-unexpected"):
                packager.validate_candidate(app, "app", expected, set())
            # Compositions without receipts, including a repeated finalization, are valid too.
            packager.discard_staging_metadata(app, expected)

    def test_staging_metadata_cannot_hide_authored_files_or_directories(self):
        for name in (".neocogi-layout.json", ".neocogi-layout.lock"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                app = Path(directory)
                path = app / name
                path.write_text("retained")
                for expected in ({name}, {name + "/child"}):
                    with self.assertRaisesRegex(ValueError, "reserved staging metadata"):
                        packager.discard_staging_metadata(app, expected)
                    self.assertEqual(path.read_text(), "retained")
                path.unlink()
                path.mkdir()
                with self.assertRaisesRegex(ValueError, "regular file"):
                    packager.discard_staging_metadata(app, set())
                self.assertTrue(path.is_dir())

    @unittest.skipUnless(os.name == 'posix', 'symlink fixture requires POSIX')
    def test_staging_metadata_symlink_is_rejected_without_touching_its_target(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            app = root / "app"
            app.mkdir()
            target = root / "retained"
            target.write_text("retained")
            for name in (".neocogi-layout.json", ".neocogi-layout.lock"):
                with self.subTest(name=name):
                    link = app / name
                    link.symlink_to(target)
                    with self.assertRaisesRegex(ValueError, "regular file"):
                        packager.discard_staging_metadata(app, set())
                    self.assertTrue(link.is_symlink())
                    self.assertEqual(target.read_text(), "retained")
                    link.unlink()


def selection(output, kind="basic", backend="glow", embedded=False, monolith=False):
    """Use the same public arguments as the documented packaging command."""
    names = {"basic": ("neocogi-demo-basic", "neocogi-demo-simple"),
             "full": ("neocogi-demo-full", "neocogi-demo-full"),
             "smoke": ("neocogi-demo-smoke", "neocogi-demo-smoke")}
    package, binary = names[kind]
    features = [backend, "monolith" if monolith else "dynamic"]
    if embedded:
        features.append("embed-assets")
    return argparse.Namespace(package=package, binary=binary, output=output, profile="dev",
                              offline=True, target=None, no_default_features=True, features=features)


def files(directory):
    """Directories have no semantic membership; compare exact relative regular files."""
    return {path.relative_to(directory).as_posix() for path in directory.rglob("*") if path.is_file()}


def wait_marker(path, process):
    """A barrier has a finite timeout and reports early worker failures."""
    deadline = time.monotonic() + 240
    while not path.exists():
        if process.poll() is not None:
            raise AssertionError(f"packaging worker exited before barrier: {process.returncode}")
        if time.monotonic() > deadline:
            process.kill()
            raise TimeoutError(f"packaging worker did not reach {path}")
        time.sleep(.05)


def worker(directory, backend, destination, tag):
    """Pause after Cargo unlocks but before copying its private exported executable.

    A second selection can now replace the canonical cache executable/providers.
    Reading that cache rather than Cargo's private export would publish mixed code.
    """
    original = packager.shutil.copy2
    def copy(source, target, *args, **kwargs):
        if Path(source).parent.name == "export":
            (directory / (tag + ".ready")).touch()
            deadline = time.monotonic() + 240
            while not (directory / (tag + ".release")).exists():
                if time.monotonic() > deadline:
                    raise TimeoutError("copy barrier timed out")
                time.sleep(.05)
        return original(source, target, *args, **kwargs)
    packager.shutil.copy2 = copy
    packager.package(selection(destination, backend=backend))


def production(directory):
    """Exercise actual macro consumers in one cache, with no cache garbage collection."""
    directory.mkdir(parents=True, exist_ok=True)
    built = {}
    for name, kind, backend, embedded, monolith in (
        ("full-a", "full", "glow", False, False),
        ("basic-a", "basic", "glow", False, False),
        ("full-b", "full", "glow", False, False),
        ("basic-vulkan", "basic", "vulkan", False, False),
        ("basic-b", "basic", "glow", False, False),
        ("embedded", "basic", "glow", True, False),
        ("monolith", "basic", "glow", False, True),
        ("smoke", "smoke", "glow", False, False),
    ):
        built[name] = packager.package(selection(directory / name, kind, backend, embedded, monolith))
        assert not ({".neocogi-layout.json", ".neocogi-layout.lock"} & files(built[name]))
    if sys.platform.startswith("linux"):
        # Old node/GFX/Skin files in the development cache cannot enter the reduced distribution.
        for name in ("full-a", "basic-a", "full-b", "basic-vulkan", "basic-b", "embedded", "smoke"):
            libraries = {path for path in files(built[name]) if path.startswith("lib/")}
            assert libraries == {"lib/libnc_ui.so", "lib/libnc_platform.so"}, (name, libraries)
        # Full enables Skin in the same UI filename; cached basic/full switches must restore it.
        assert digest(built["full-a"] / "lib/libnc_ui.so") == digest(built["full-b"] / "lib/libnc_ui.so")
        assert digest(built["basic-a"] / "lib/libnc_ui.so") == digest(built["basic-b"] / "lib/libnc_ui.so")
        assert digest(built["full-a"] / "lib/libnc_ui.so") != digest(built["basic-a"] / "lib/libnc_ui.so")
    assert files(built["full-a"]) == files(built["full-b"])
    assert files(built["basic-a"]) == files(built["basic-b"])
    assert not any(name.startswith("themes/") or "FACEPALM" in name or name.endswith(".obj") for name in files(built["basic-a"]))
    assert not any(name.startswith(("themes/", "assets/")) for name in files(built["embedded"]))
    assert not any(name.startswith("lib/") for name in files(built["monolith"]))
    provider = "lib/libnc_platform.so" if sys.platform.startswith("linux") else "lib/nc_platform.dll" if os.name == "nt" else "lib/libnc_platform.dylib"
    assert digest(built["basic-a"] / provider) == digest(built["basic-b"] / provider)
    assert digest(built["basic-vulkan"] / provider) != digest(built["basic-b"] / provider)

    def spawn(backend, output, tag):
        log = (directory / (tag + ".log")).open("w")
        process = subprocess.Popen([sys.executable, __file__, "--worker", str(directory), backend, str(output), tag], stdout=log, stderr=log)
        log.close()
        return process

    first = spawn("glow", directory / "concurrent-a", "a")
    wait_marker(directory / "a.ready", first)
    # Building B while A is paused deterministically changes the shared canonical files.
    other = packager.package(selection(directory / "concurrent-b", backend="vulkan"))
    (directory / "a.release").touch()
    assert first.wait(timeout=60) == 0
    executable = "neocogi-demo-simple" + (".exe" if os.name == "nt" else "")
    for relative in (executable, provider):
        assert digest(directory / "concurrent-a" / relative) == digest(built["basic-b"] / relative)
        assert digest(other / relative) == digest(built["basic-vulkan"] / relative)
    destination = directory / "one-winner"
    left, right = spawn("glow", destination, "left"), spawn("vulkan", destination, "right")
    try:
        wait_marker(directory / "left.ready", left)
        wait_marker(directory / "right.ready", right)
        (directory / "left.release").touch()
        (directory / "right.release").touch()
        results = [left.wait(timeout=60), right.wait(timeout=60)]
        assert sorted(results) == [0, 1], results
        winner = built["basic-b"] if results[0] == 0 else built["basic-vulkan"]
        for relative in (executable, provider):
            assert digest(destination / relative) == digest(winner / relative)
    finally:
        for process in (left, right):
            if process.poll() is None:
                process.kill()
                process.wait()
    print("Production file sets, cached backend switching, private executable capture and single-winner publication passed")
    recipe_fixture(directory)


def recipe_fixture(directory):
    """Removed optional inputs and missing required inputs cross real production build APIs.

    A small authored Cargo fixture isolates deletions from the user's actual assets.
    It shares the repository target cache and pinned configuration, without a copied
    resolver or any production packager special case.
    """
    with tempfile.TemporaryDirectory(prefix="neocogi-resource-fixture-") as temporary:
        fixture = Path(temporary)
        (fixture / ".cargo").mkdir()
        shutil.copy2(ROOT / ".cargo/config.toml", fixture / ".cargo/config.toml")
        shutil.copy2(ROOT / "rust-toolchain.toml", fixture / "rust-toolchain.toml")
        (fixture / "Cargo.toml").write_text('[workspace]\nmembers=["app","assets"]\nresolver="3"\n')
        for name in ("app", "assets"):
            (fixture / name / "src").mkdir(parents=True)
        from contract_check_support import registry_dependency
        layout = registry_dependency(ROOT, "neocogi-platform-contracts", ["build-support"])
        (fixture / "app/Cargo.toml").write_text('[package]\nname="recipe-fixture"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nneocogi-demo-assets={path="../assets"}\n')
        (fixture / "app/src/main.rs").write_text('fn main(){neocogi_demo_assets::ready();}\n')
        (fixture / "assets/Cargo.toml").write_text('[package]\nname="neocogi-demo-assets"\nversion="0.0.0"\nedition="2024"\n[build-dependencies]\n' + layout)
        (fixture / "assets/src/lib.rs").write_text('pub fn ready(){}\n')
        (fixture / "assets/required.txt").write_text("required")
        (fixture / "assets/optional.txt").write_text("optional")
        (fixture / "assets/build.rs").write_text(r'''
fn main() {
    println!("cargo:rerun-if-changed=.");
    use neocogi_platform_contracts::build::layout::{Resource, output_root, stage_resources};
    use std::path::PathBuf;
    let mut resources=Vec::new();
    let mut paths=Vec::new();
    for name in ["required.txt","optional.txt"] {
        if name=="required.txt" || std::path::Path::new(name).is_file() {
            resources.push(Resource { source: PathBuf::from(name), destination: PathBuf::from(name) });
            paths.push(format!("{name:?}"));
        }
    }
    let anchor=PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-env-changed=NEOCOGI_NATIVE_OUTPUT_DIR");
    let root=output_root(Some(&anchor),std::env::var_os("NEOCOGI_NATIVE_OUTPUT_DIR").map(Into::into)).unwrap();
    let staged=stage_resources(&PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()),&root,&resources).unwrap();
    for path in staged.inputs.iter().chain(&staged.outputs) { println!("cargo:rerun-if-changed={}",path.display()); }
    println!("cargo:rustc-env=NEOCOGI_RESOURCE_FILES=[{}]",paths.join(","));
}
''')
        environment = {**os.environ, "CARGO_TARGET_DIR": str(ROOT / "target")}
        subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=fixture, env=environment, check=True)
        original_root, original_target = packager.ROOT, os.environ.get("CARGO_TARGET_DIR")
        packager.ROOT = fixture
        os.environ["CARGO_TARGET_DIR"] = str(ROOT / "target")
        args = selection(directory / "recipe-first")
        args.package, args.binary, args.features = "recipe-fixture", "recipe-fixture", []
        try:
            first = packager.package(args)
            assert {"required.txt", "optional.txt"} <= files(first)
            # Deleted staged outputs must restore on the normal cached build path.
            command = ["cargo", "build", "--locked", "--offline", "-p", "recipe-fixture"]
            subprocess.run(command, cwd=fixture, env=environment, check=True)
            staged = ROOT / "target/debug/required.txt"
            assert staged.read_text() == "required"
            staged.unlink()
            subprocess.run(command, cwd=fixture, env=environment, check=True)
            assert staged.read_text() == "required"
            args.output = directory / "recipe-cached"
            assert "required.txt" in files(packager.package(args))
            (fixture / "assets/optional.txt").unlink()
            args.output = directory / "recipe-removed"
            assert "optional.txt" not in files(packager.package(args))
            (fixture / "assets/required.txt").unlink()
            args.output = directory / "recipe-failed"
            try:
                packager.package(args)
            except RuntimeError:
                pass
            else:
                raise AssertionError("missing required resource was published")
            assert not args.output.exists()
            assert (first / "optional.txt").read_text() == "optional"
        finally:
            packager.ROOT = original_root
            if original_target is None:
                os.environ.pop("CARGO_TARGET_DIR", None)
            else:
                os.environ["CARGO_TARGET_DIR"] = original_target
    print("Production resource staging deletion, required-input failure and prior-output preservation passed")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--worker":
        worker(Path(sys.argv[2]), sys.argv[3], Path(sys.argv[4]), sys.argv[5])
    elif len(sys.argv) > 1 and sys.argv[1] == "--production":
        production(Path(sys.argv[2]))
    else:
        unittest.main()
