"""Child environment for the two independent checkout-contract checks.

The external consumer directories are outside the repository toolchain override.
Resolve that selection before changing cwd; preserve explicit caller overrides
and the meaning of a caller-relative build cache without mutating os.environ.
Cargo owns dependency fetching; callers can opt into offline operation with
CARGO_NET_OFFLINE when their dependencies are already cached.
"""

import json
import os
from pathlib import Path
import subprocess
import tomllib


def registry_dependency(repository, package, features=()):
    """Use the workspace's pinned COM package in an independent Cargo graph.

    Standalone fixtures cannot inherit workspace dependencies. Copy only the
    exact registry version and default-feature policy, then select their own features.
    This preserves Rust package identity across local UI and generated bindings.
    """
    with (repository / "Cargo.toml").open("rb") as source:
        dependency = tomllib.load(source)["workspace"]["dependencies"][package]
    version = dependency.get("version", "")
    if not isinstance(version, str) or not version.startswith("=") or any(
        key in dependency for key in ("git", "rev", "path", "registry", "registry-index", "branch", "tag")
    ):
        raise ValueError(f"{package}: fixtures require an exact crates.io version")
    declaration = {"version": version}
    if "default-features" in dependency:
        declaration["default-features"] = dependency["default-features"]
    declaration["features"] = list(features)
    return package + "={" + ",".join(key + "=" + json.dumps(value) for key, value in declaration.items()) + "}\n"


def child_environment(repository):
    """Carry the selected compiler, caller cache and Cargo network policy to children."""
    environment = dict(os.environ)
    selected = subprocess.check_output(
        ["rustup", "show", "active-toolchain"], cwd=repository,
        env=environment, text=True,
    ).split()[0]
    environment["RUSTUP_TOOLCHAIN"] = selected
    if "CARGO_TARGET_DIR" in environment:
        environment["CARGO_TARGET_DIR"] = str(Path(environment["CARGO_TARGET_DIR"]).resolve())
    return environment
