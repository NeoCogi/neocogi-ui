#!/usr/bin/env python3
"""Build one Cargo-selected application into a fresh, exact native distribution.

Shared Cargo outputs remain an additive development cache. Cargo exports the chosen
executable under its own build coordination; macros copy selected providers/resources
into private candidate storage. Only a validated candidate is renamed into place.
Concurrent use requires Cargo-supported local filesystem locking. No wrapper build
lock, second resolver, generated workspace or cache garbage collection is used.
"""
import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
RESOURCE_ENV = "NEOCOGI_RESOURCE_FILES"
LIBRARY_SUFFIXES = {".so", ".dll", ".dylib"}
# Platform owns these reserved root names for build-time resource ownership and locking.
STAGING_METADATA = frozenset({".neocogi-layout.json", ".neocogi-layout.lock"})


def cargo_options(args):
    """Keep source/toolchain ownership with the workspace-pinned, locked Cargo invocation."""
    return ["--locked", *(["--offline"] if args.offline else [])]


def select_package(args):
    """Select one authored workspace binary from Cargo metadata without resolving its dependencies."""
    command = ["cargo", "metadata", "--no-deps", "--format-version=1", *cargo_options(args)]
    metadata = json.loads(subprocess.check_output(command, cwd=ROOT, text=True))
    matches = [package for package in metadata["packages"] if args.package in (package["name"], package["id"])]
    if len(matches) != 1:
        raise ValueError("--package must select exactly one workspace package")
    package = matches[0]
    if not any(target["name"] == args.binary and "bin" in target["kind"] for target in package["targets"]):
        raise ValueError(f"{package['name']} does not declare binary {args.binary}")
    assets = {package["id"] for package in metadata["packages"] if package["name"] == "neocogi-demo-assets"}
    return package, assets


def relative_resource(value):
    """Admit build-reported application paths without absolute paths or parent traversal."""
    path = PurePosixPath(value)
    if not value or path.is_absolute() or ".." in path.parts or "\\" in value or ":" in value:
        raise ValueError(f"invalid packaged resource path: {value!r}")
    return path.as_posix()


def build_candidate(args, package, assets, app, export):
    """Capture exact build metadata and the executable exported before Cargo releases its lock.

    Expected resources come from the selected asset recipe's build-script env event;
    expected DLLs come from actual cdylib artifacts. Neither is guessed from features
    or copied from broad shared-cache directories after the build.
    """
    command = ["cargo", "build", *cargo_options(args), "-p", package["name"], "--bin", args.binary,
               "--profile", args.profile, "--message-format=json-render-diagnostics",
               "--artifact-dir", str(export), "-Zunstable-options"]
    if args.no_default_features:
        command.append("--no-default-features")
    if args.features:
        command.extend(["--features", ",".join(args.features)])
    if args.target:
        command.extend(["--target", args.target])
    environment = {**os.environ, "NEOCOGI_NATIVE_OUTPUT_DIR": str(app)}
    executable = None
    libraries = set()
    resource_sets = []
    uses_assets = False
    with subprocess.Popen(command, cwd=ROOT, env=environment, stdout=subprocess.PIPE, text=True) as process:
        try:
            for line in process.stdout:
                message = json.loads(line)
                reason = message.get("reason")
                if reason == "build-script-executed" and message["package_id"] in assets:
                    values = dict(message.get("env", []))
                    if RESOURCE_ENV not in values:
                        raise ValueError("selected asset build omitted its resource file set")
                    resource_sets.append({relative_resource(path) for path in json.loads(values[RESOURCE_ENV])})
                if reason != "compiler-artifact":
                    continue
                uses_assets |= message["package_id"] in assets
                if message["target"]["kind"] == ["cdylib"]:
                    libraries.update("lib/" + Path(path).name for path in message["filenames"] if Path(path).suffix in LIBRARY_SUFFIXES)
                if message["package_id"] == package["id"] and message["target"]["name"] == args.binary and "bin" in message["target"]["kind"]:
                    if not message.get("executable") or executable is not None:
                        raise ValueError("Cargo did not report one unambiguous selected executable")
                    executable = Path(message["executable"]).name
            if process.wait() != 0:
                raise RuntimeError("Cargo build failed; no distribution was published")
        except BaseException:
            # A malformed metadata event must not leave Cargo blocked on a full stdout
            # pipe while the Popen context waits for it. Nothing can be published here.
            process.kill()
            process.communicate()
            raise
    if executable is None or not (export / executable).is_file():
        raise ValueError("Cargo did not export the selected executable")
    if uses_assets and not resource_sets:
        raise ValueError("selected assets have no build-reported resource set")
    if resource_sets and any(paths != resource_sets[0] for paths in resource_sets):
        raise ValueError("ambiguous asset roles reported different resource sets")
    # Export is private scratch, never the mutable canonical executable in Cargo's cache.
    # copy2 makes a new file and preserves executable mode; no final hardlink escapes.
    shutil.copy2(export / executable, app / executable)
    resources = resource_sets[0] if resource_sets else set()
    return executable, libraries | resources | {executable}, libraries


def inspect_loader(path):
    """Inspect native loader metadata with the target's standard binary tool; fail closed if absent."""
    magic = path.read_bytes()[:4]
    if magic == b"\x7fELF":
        tool = shutil.which("readelf")
        if tool is None:
            raise ValueError("ELF package inspection requires readelf")
        output = subprocess.check_output([tool, "-d", str(path)], text=True)
        for line in output.splitlines():
            if "(NEEDED)" in line and "/" in line.split("[", 1)[1].split("]", 1)[0]:
                raise ValueError(f"absolute native dependency in {path.name}")
            if "RPATH" in line or "RUNPATH" in line:
                entries = line.split("[", 1)[1].split("]", 1)[0].split(":")
                if not all(entry in ("$ORIGIN", "$ORIGIN/lib") for entry in entries):
                    raise ValueError(f"non-relocatable loader path in {path.name}")
    elif magic[:2] == b"MZ":
        tool = shutil.which("llvm-readobj")
        if tool is None:
            raise ValueError("PE package inspection requires llvm-readobj")
        output = subprocess.check_output([tool, "--coff-imports", str(path)], text=True)
        for line in output.splitlines():
            if line.strip().startswith("Name:"):
                name = line.split(":", 1)[1].strip()
                if any(character in name for character in ("/", "\\", ":")):
                    raise ValueError(f"non-relocatable PE dependency in {path.name}")
    elif magic in (b"\xcf\xfa\xed\xfe", b"\xfe\xed\xfa\xcf", b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca"):
        tool = shutil.which("otool")
        if tool is None:
            raise ValueError("Mach-O package inspection requires otool")
        output = subprocess.check_output([tool, "-L", str(path)], text=True)
        for line in output.splitlines()[1:]:
            name = line.strip().split(" (", 1)[0]
            if not name.startswith(("/usr/lib/", "/System/Library/", "@rpath/", "@loader_path/", "@executable_path/")) or ".." in PurePosixPath(name).parts:
                raise ValueError(f"non-relocatable Mach-O dependency in {path.name}")
        load_commands = subprocess.check_output([tool, "-l", str(path)], text=True).splitlines()
        for index, line in enumerate(load_commands):
            if line.strip() == "cmd LC_RPATH":
                entry = load_commands[index + 2].strip().split(" (", 1)[0].removeprefix("path ")
                if entry not in ("@loader_path", "@loader_path/lib", "@executable_path", "@executable_path/lib"):
                    raise ValueError(f"non-relocatable Mach-O search path in {path.name}")
    else:
        raise ValueError(f"unsupported native executable format: {path.name}")
    if any(name in output.lower() for name in ("libnc_com", "nc_com.dll", "libneocogi_com_host")):
        raise ValueError(f"COM services must be supplied by the executable's host: {path.name}")


def discard_staging_metadata(app, expected):
    """Remove only Platform's build receipts after Cargo and all its compiler children exit.

    Each invocation owns this fresh candidate exclusively. Its lock inode can now be
    discarded safely; shared Cargo outputs keep their receipts and persistent locks.
    Authored paths cannot claim these reserved roots, and malformed metadata shapes
    must fail rather than disappearing before the ordinary exact-membership check.
    """
    for name in STAGING_METADATA:
        if any(path == name or path.startswith(name + "/") for path in expected):
            raise ValueError(f"application files claim reserved staging metadata: {name}")
        path = app / name
        if path.is_symlink() or (path.exists() and not path.is_file()):
            raise ValueError(f"staging metadata must be a regular file: {name}")
    for name in STAGING_METADATA:
        (app / name).unlink(missing_ok=True)


def validate_candidate(app, executable, expected, libraries):
    """Require exact membership, private-path-free binaries and relocatable loader metadata."""
    paths = list(app.rglob("*"))
    if any(path.is_symlink() for path in paths):
        raise ValueError("a package candidate must contain owned regular files, not symlinks")
    actual = {path.relative_to(app).as_posix() for path in paths if path.is_file()}
    if actual != expected:
        raise ValueError(f"package membership differs: missing={sorted(expected - actual)}, unexpected={sorted(actual - expected)}")
    private = {str(ROOT), str(Path.home()), str(app.parent)}
    private.update(str((ROOT / os.environ[key]).resolve())
                   for key in ("CARGO_HOME", "RUSTUP_HOME", "CARGO_TARGET_DIR", "CARGO_BUILD_BUILD_DIR")
                   if os.environ.get(key))
    for relative in libraries | {executable}:
        path = app / relative
        data = path.read_bytes()
        for value in private:
            for encoding in ("utf-8", "utf-16-le"):
                if value.encode(encoding) in data:
                    raise ValueError(f"private build path in {relative}")
        inspect_loader(path)


@contextmanager
def destination_lock(destination):
    """Serialize final publication by destination name, without locking or owning Cargo's cache.

    Keep the lock file beside the destination: removing it would permit waiters to
    lock different inodes. The protocol coordinates packagers, not arbitrary writers.
    """
    with (destination.parent / ("." + destination.name + ".lock")).open("a+b") as lock:
        if os.name == "nt":
            import msvcrt
            lock.seek(0, os.SEEK_END)
            if lock.tell() == 0:
                lock.write(b"\0")
                lock.flush()
            lock.seek(0)
            msvcrt.locking(lock.fileno(), msvcrt.LK_LOCK, 1)
            try:
                yield
            finally:
                lock.seek(0)
                msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
        elif os.name == "posix":
            import fcntl
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
            try:
                yield
            finally:
                fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
        else:
            raise ValueError("destination publication locking is unsupported on this platform")


def package(args):
    """Own scratch on the destination filesystem and publish one complete new application."""
    requested = args.output.expanduser().absolute()
    requested.parent.mkdir(parents=True, exist_ok=True)
    destination = requested.parent.resolve() / requested.name
    if os.path.lexists(destination):
        raise FileExistsError(f"distribution destination already exists: {destination}")
    selected, assets = select_package(args)
    with tempfile.TemporaryDirectory(prefix="." + destination.name + ".build-", dir=destination.parent) as scratch:
        scratch = Path(scratch)
        app, export = scratch / "app", scratch / "export"
        app.mkdir()
        export.mkdir()
        executable, expected, libraries = build_candidate(args, selected, assets, app, export)
        discard_staging_metadata(app, expected)
        validate_candidate(app, executable, expected, libraries)
        with destination_lock(destination):
            if os.path.lexists(destination):
                raise FileExistsError(f"distribution destination already exists: {destination}")
            app.rename(destination)
    return destination


def main():
    """Package an exact Cargo selection; an existing published application is never replaced."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True, help="workspace package name")
    parser.add_argument("--bin", dest="binary", required=True, help="one declared executable target")
    parser.add_argument("--output", required=True, type=Path, help="new distribution directory")
    parser.add_argument("--profile", default="dev")
    parser.add_argument("--features", action="append", default=[])
    parser.add_argument("--no-default-features", action="store_true")
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--target", help="Cargo deployment target; requires target inspection tools")
    args = parser.parse_args()
    try:
        print(package(args))
    except (ValueError, RuntimeError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Packaging failed: {error}\n")


if __name__ == "__main__":
    main()
