#!/usr/bin/env python3
"""Qualify real UI release archives and external consumers without uploading packages.

Packaging resolves unpublished members locally without compiling checkout source.
All subsequent internal code comes exclusively from inspected, extracted archives.
Archive builds and their fixture children share one fresh external target directory,
so generated Cargo workspaces cannot inherit the checkout's development configuration.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

from package_inputs import reconcile

ROOT = Path(__file__).resolve().parents[1]
RELEASE = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
SCHEMA_OWNER = "neocogi-ui-contracts"
PUBLIC_PACKAGES = {"neocogi-ui-contracts", "neocogi-ui", "neocogi-ui-dylib"}


def run(arguments, *, cwd=ROOT, env=None, capture=False):
    """Sequence one foreground command and preserve its concrete failure and diagnostics."""
    print("+ " + " ".join(str(arg) for arg in arguments), flush=True)
    if capture:
        return subprocess.check_output(arguments, cwd=cwd, env=env, text=True)
    subprocess.run(arguments, cwd=cwd, env=env, check=True)


def release_packages(metadata):
    """Admit only the documented reusable package set and an exact registry dependency closure."""
    members = {p["id"]: p for p in metadata["packages"] if p["id"] in metadata["workspace_members"]}
    selected = {}
    for package in members.values():
        path = Path(package["manifest_path"]).relative_to(ROOT)
        public = path.parts[0] == "crates"
        if package["version"] != RELEASE:
            raise ValueError(f"workspace release version differs: {package['name']}")
        if package["publish"] != (["crates-io"] if public else []):
            raise ValueError(f"unexpected publication policy: {package['name']}")
        if public:
            selected[package["name"]] = package
    if set(selected) != PUBLIC_PACKAGES:
        raise ValueError("release must contain exactly contracts, static UI and UI dylib")
    for package in selected.values():
        for dependency in package["dependencies"]:
            if not dependency.get("path"):
                continue
            if dependency["name"] not in selected:
                raise ValueError(f"release depends on private package: {package['name']} -> {dependency['name']}")
            expected = "=" + RELEASE
            if dependency["req"] != expected:
                raise ValueError(f"internal dependency is not exact: {package['name']} -> {dependency['name']}")
    return selected


def archive_manifest(archive, package):
    """Reject unsafe paths, missing inputs and nonregistry normalized dependencies before extraction."""
    prefix = package["name"] + "-" + package["version"]
    with tarfile.open(archive) as bundle:
        members = bundle.getmembers()
        names = set()
        for member in members:
            path = PurePosixPath(member.name)
            if path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] != prefix:
                raise ValueError(f"unsafe archive path: {member.name}")
            if not (member.isfile() or member.isdir()) or member.name in names:
                raise ValueError(f"nonregular or repeated archive member: {member.name}")
            names.add(member.name)
        for required in ["Cargo.toml", "README.md", "LICENSE"]:
            if f"{prefix}/{required}" not in names:
                raise ValueError(f"missing archive input: {package['name']}/{required}")
        manifest = tomllib.loads(bundle.extractfile(f"{prefix}/Cargo.toml").read().decode())
        identity = manifest["package"]
        if identity["name"] != package["name"] or identity["version"] != RELEASE:
            raise ValueError("archive identity differs from selected release")
        for table in [manifest, *manifest.get("target", {}).values()]:
            for group in ["dependencies", "build-dependencies", "dev-dependencies"]:
                for name, dependency in table.get(group, {}).items():
                    if any(key in dependency for key in ["git", "path", "workspace", "registry", "registry-index"]):
                        raise ValueError(f"nonregistry archive dependency: {package['name']} -> {name}")
                    if not dependency.get("version"):
                        raise ValueError(f"unversioned archive dependency: {name}")
        shared = [name for name in names if name.startswith(prefix + "/schemas/neocogi/") and name.endswith(".ncidl")]
        expected = 32 if package["name"] == SCHEMA_OWNER else 0
        if len(shared) != expected:
            raise ValueError(f"incorrect shared schema ownership: {package['name']}")
        if any("/scripts/" in name or "/.agents/" in name or "/target/" in name for name in names):
            raise ValueError("repository-only files leaked into archive")
    return prefix


def patch_configuration(packages, locations):
    """Resolve exactly this unpublished release from explicit local contents; no other sources change."""
    return "[patch.crates-io]\n" + "".join(name + "={path=" + json.dumps(str(locations[name])) + "}\n" for name in sorted(packages))


def provider_libraries(artifacts):
    """Select final shared libraries, never similarly described Cargo test executables."""
    libraries = {}
    for line in artifacts.splitlines():
        record = json.loads(line)
        if (record.get("reason") != "compiler-artifact" or record.get("profile", {}).get("test")
                or "cdylib" not in record["target"]["crate_types"]):
            continue
        name = record["package_id"].rsplit("#", 1)[-1].split("@", 1)[0]
        shared = [path for path in record["filenames"] if Path(path).suffix in {".so", ".dylib", ".dll"}]
        if len(shared) != 1:
            raise ValueError(f"expected exactly one final provider library: {name}")
        libraries[name] = shared[0]
    return libraries


def archive_command(cargo, packages, flags, resolution, *, allow_dirty, publish_dry_run):
    """Construct a package or nonuploading publish preflight over exactly the admitted release."""
    command = [*cargo, "publish", "--dry-run"] if publish_dry_run else [*cargo, "package"]
    command += ["--locked", "--no-verify", *flags, "--config", str(resolution)]
    if allow_dirty: command.append("--allow-dirty")
    for name in sorted(packages): command += ["-p", name]
    return command


def validation_environment(outside, packages, locations, toolchain, *, offline):
    """Own archive-only configuration and a fresh target outside the development checkout.

    Trybuild creates child Cargo workspaces below CARGO_TARGET_DIR. Keeping that
    directory in the checkout would let those children inherit .cargo/config.toml,
    including build-std, even though their parent test ran against extracted sources.
    Registry/Git caches may be shared; build policy and compiled outputs must not be.
    """
    if outside.resolve().is_relative_to(ROOT.resolve()):
        raise ValueError("archive validation directory must be outside the checkout")
    home = outside / "cargo-home"
    home.mkdir()
    original_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    for name in ["registry", "git"]:
        if (original_home / name).exists():
            (home / name).symlink_to(original_home / name, target_is_directory=True)
    (home / "config.toml").write_text(patch_configuration(packages, locations))
    environment = dict(os.environ, CARGO_HOME=str(home), CARGO_TARGET_DIR=str(outside / "target"), RUSTUP_TOOLCHAIN=toolchain)
    if offline:
        environment["CARGO_NET_OFFLINE"] = "true"
    return environment


def qualify(*, offline=False, allow_dirty=False, stable=None, output=None, publish_dry_run=False):
    """Build actual archives, inspect them and execute only extracted internal implementation code."""
    if offline and publish_dry_run:
        raise ValueError("Cargo publication preflight needs registry access; omit --offline or use package mode")
    if not allow_dirty and run(["git", "status", "--porcelain"], capture=True).strip():
        raise ValueError("release qualification requires a clean committed tree; --allow-dirty is for development")
    if output and output.resolve().exists():
        raise ValueError(f"release output already exists: {output.resolve()}")
    reconcile(check=True)
    flags = ["--offline"] if offline else []
    toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    cargo = ["cargo", "+" + toolchain]
    metadata = json.loads(run([*cargo, "metadata", "--locked", "--format-version=1", *flags], capture=True))
    packages = release_packages(metadata)
    packaging_target = Path(metadata["target_directory"])
    with tempfile.TemporaryDirectory(prefix="neocogi-ui-release-") as temporary:
        outside = Path(temporary)
        resolution = outside / "packaging.toml"
        resolution.write_text(patch_configuration(packages, {name: Path(p["manifest_path"]).parent for name, p in packages.items()}))
        command = archive_command(cargo, packages, flags, resolution, allow_dirty=allow_dirty, publish_dry_run=publish_dry_run)
        # No checkout code is compiled here. This resolution-only patch handles
        # exact internal registry edges before registry versions are uploaded.
        run(command)
        # The pinned workspace publisher retains its upload candidates separately
        # from `cargo package` output; never inspect stale candidates from the other mode.
        archive_directory = packaging_target / "package"
        if publish_dry_run: archive_directory /= "tmp-crate"
        locations = {}
        inventory = {}
        for name, package in sorted(packages.items()):
            archive = archive_directory / f"{name}-{RELEASE}.crate"
            prefix = archive_manifest(archive, package)
            relative = Path(package["manifest_path"]).parent.relative_to(ROOT)
            parent = outside / relative.parent
            parent.mkdir(parents=True, exist_ok=True)
            with tarfile.open(archive) as bundle:
                bundle.extractall(parent, filter="data")
            destination = outside / relative
            (parent / prefix).rename(destination)
            locations[name] = destination
            inventory[archive.name] = hashlib.sha256(archive.read_bytes()).hexdigest()
        shutil.copytree(ROOT / "scripts/package-consumer", outside / "consumer")
        members = [str(path.relative_to(outside)) for path in locations.values()] + ["consumer"]
        manifest = "[workspace]\nresolver='3'\nmembers=" + json.dumps(members) + "\n" + patch_configuration(packages, locations)
        (outside / "Cargo.toml").write_text(manifest)
        shutil.copyfile(ROOT / "Cargo.lock", outside / "Cargo.lock")
        environment = validation_environment(outside, packages, locations, toolchain, offline=offline)
        # These commands have no repository .cargo/config.toml, scripts, assets,
        # authored schemas or source paths outside the extracted release set.
        artifacts = run([*cargo, "build", "--workspace", "--lib", "--bins", "--all-features", "--message-format=json", *flags], cwd=outside, env=environment, capture=True)
        libraries = provider_libraries(artifacts)
        run([*cargo, "run", "-p", "neocogi-ui-package-consumer", *flags], cwd=outside, env=environment)
        required = ["neocogi-ui-dylib"]
        if any(name not in libraries for name in required):
            raise ValueError("real archived provider libraries missing from Cargo artifact output")
        run([*cargo, "run", "-p", "neocogi-ui-package-consumer", *flags, "--", *[libraries[name] for name in required]], cwd=outside, env=environment)
        run([*cargo, "clippy", "-p", "neocogi-ui-package-consumer", "--all-targets", *flags, "--", "-D", "warnings"], cwd=outside, env=environment)
        if stable:
            stable_environment = dict(environment, RUSTUP_TOOLCHAIN=stable)
            run(["cargo", "+" + stable, "run", "-p", "neocogi-ui-package-consumer", *flags], cwd=outside, env=stable_environment)
        run([*cargo, "test", "--workspace", "--all-features", *flags], cwd=outside, env=environment)
        if output:
            output = output.resolve()
            output.mkdir(parents=True)
            for filename in inventory: shutil.copyfile(archive_directory / filename, output / filename)
            (output / "SHA256SUMS").write_text("".join(digest + "  " + name + "\n" for name, digest in sorted(inventory.items())))
    print(f"PASS: {len(packages)} inspected archives, extracted tests and static/dynamic external consumers", flush=True)


def main():
    """Network, dirty-tree, stable compiler and retained archive policies are explicit caller choices."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--allow-dirty", action="store_true")
    parser.add_argument("--stable-toolchain")
    parser.add_argument("--publish-dry-run", action="store_true", help="Use Cargo publish --dry-run to create archives; never upload")
    parser.add_argument("--output", type=Path)
    options = parser.parse_args()
    qualify(offline=options.offline, allow_dirty=options.allow_dirty, stable=options.stable_toolchain,
            output=options.output, publish_dry_run=options.publish_dry_run)


if __name__ == "__main__":
    main()
