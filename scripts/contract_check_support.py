"""Child environment for the two independent checkout-contract checks.

The external consumer directories are outside the repository toolchain override.
Resolve that selection before changing cwd; preserve explicit caller overrides
and the meaning of a caller-relative build cache without mutating os.environ.
"""

import os
from pathlib import Path
import subprocess


def child_environment(repository):
    """Carry the repository-selected compiler and caller cache to all children."""
    environment = dict(os.environ, CARGO_NET_OFFLINE="true")
    selected = subprocess.check_output(
        ["rustup", "show", "active-toolchain"], cwd=repository,
        env=environment, text=True,
    ).split()[0]
    environment["RUSTUP_TOOLCHAIN"] = selected
    if "CARGO_TARGET_DIR" in environment:
        environment["CARGO_TARGET_DIR"] = str(Path(environment["CARGO_TARGET_DIR"]).resolve())
    return environment
