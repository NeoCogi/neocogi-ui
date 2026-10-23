#!/usr/bin/env python3
"""Cargo test runner: execute native acceptance fixtures in a copied app layout.

Cargo places test executables in nested compiler directories. Production lookup
stays executable-relative, so acceptance tests need their adjacent files too.
This runner only copies and executes an already compiled test; it never builds.
"""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def main():
    """Copy the test and current staged inputs, preserving the test's exit status."""
    executable = Path(sys.argv[1]).resolve()
    explicit = os.environ.get("NEOCOGI_NATIVE_OUTPUT_DIR")
    root = Path(explicit) if explicit else next(
        directory for directory in executable.parents if (directory / ".cargo-lock").is_file()
    )
    with tempfile.TemporaryDirectory(prefix="neocogi test app ") as directory:
        application = Path(directory)
        shutil.copy2(executable, application / executable.name)
        for name in ("lib", "assets", "themes"):
            if (root / name).is_dir():
                shutil.copytree(root / name, application / name)
        environment = {key: value for key, value in os.environ.items()
                       if key not in ("LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH", "DYLD_FALLBACK_LIBRARY_PATH")}
        result = subprocess.run([application / executable.name, *sys.argv[2:]], env=environment, check=False)
        return result.returncode if result.returncode >= 0 else 128 - result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
