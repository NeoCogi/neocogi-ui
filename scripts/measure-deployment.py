#!/usr/bin/env python3
"""Measure one fresh native distribution and optionally compare its build configuration.

Count the executable and selected providers under lib/, including the shader
compiler, once each. Assets/debug sidecars are excluded. Hashes and file sizes
are portable; GNU size section measurements are optional. Fresh directories and
changed provider membership do not prevent an architectural comparison.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def capture_file(path):
    """Retain exact file identity; use optional GNU size only for ELF section detail."""
    sections = {}
    data = path.read_bytes()
    if data[:4] == b"\x7fELF" and shutil.which("size"):
        result = subprocess.run(["size", "-A", str(path)], capture_output=True, text=True)
        if result.returncode == 0:
            for line in result.stdout.splitlines():
                match = re.fullmatch(r"(\S+)\s+(\d+)\s+(\d+)", line.strip())
                if match:
                    sections[match[1]] = int(match[2])
    return {
        "bytes": path.stat().st_size,
        "sha256": hashlib.sha256(data).hexdigest(),
        "sections": sections,
    }


def main():
    """Write auditable measurements before enforcing the optional size limit."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path, help="staged directory containing the executable and shared libraries")
    parser.add_argument("--executable", required=True, help="executable filename within that directory")
    parser.add_argument("--output", required=True, type=Path, help="JSON record to preserve across cargo clean")
    parser.add_argument("--baseline", type=Path, help="previous JSON from the same build configuration")
    parser.add_argument("--configuration", required=True, help="comparison key describing profile, target, features and workload")
    parser.add_argument("--max-growth", type=int, default=0, help="allowed total byte growth when comparing (default: zero)")
    args = parser.parse_args()
    if Path(args.executable).name != args.executable or not (args.directory / args.executable).is_file():
        parser.error("--executable must name an existing file directly inside the staged directory")
    paths = [args.directory / args.executable]
    if (args.directory / "lib").exists():
        paths.extend(sorted(path for path in (args.directory / "lib").iterdir() if path.suffix in (".so", ".dll", ".dylib")))
    if any(path.is_symlink() or not path.is_file() for path in paths):
        parser.error("measurement requires owned distribution files, without symlinks")
    files = {path.relative_to(args.directory).as_posix(): capture_file(path) for path in paths}
    record = {
        "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "working_tree": subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).splitlines(),
        "rustc": subprocess.check_output(["rustc", "--version"], cwd=ROOT, text=True).strip(),
        "configuration": args.configuration,
        "directory": str(args.directory.resolve()),
        "files": files,
        "total_bytes": sum(item["bytes"] for item in files.values()),
    }
    growth = None
    if args.baseline:
        baseline = json.loads(args.baseline.read_text())
        if baseline["rustc"] != record["rustc"] or baseline.get("configuration") != record["configuration"]:
            parser.error("baseline and candidate must use the same compiler and comparison configuration")
        growth = record["total_bytes"] - baseline["total_bytes"]
        record["baseline"] = str(args.baseline)
        record["growth_bytes"] = growth
        record["added_files"] = sorted(files.keys() - baseline["files"].keys())
        record["removed_files"] = sorted(baseline["files"].keys() - files.keys())
    args.output.write_text(json.dumps(record, indent=2) + "\n")
    for name, item in files.items():
        print(f"{name}: {item['bytes']:,} bytes")
    print(f"Total: {record['total_bytes']:,} bytes")
    if growth is not None:
        print(f"Change: {growth:+,} bytes; allowed growth: {args.max_growth:,}")
        if growth > args.max_growth:
            raise SystemExit("deployment exceeds the size budget")


if __name__ == "__main__":
    main()
