#!/usr/bin/env python3
"""Record a staged Linux deployment and reject growth against a saved baseline.

Build both revisions with the same compiler, profile, feature set, workspace and
output path before invoking this script. It measures the executable and top-level
.so files once each, excluding assets and the separate runtime linking directory.
GNU size reports ELF section contributions even when deployment files are stripped.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess


def capture_file(path):
    """Retain exact file identity and ELF section sizes without copying binaries."""
    sections = {}
    output = subprocess.check_output(["size", "-A", str(path)], text=True)
    for line in output.splitlines():
        match = re.fullmatch(r"(\S+)\s+(\d+)\s+(\d+)", line.strip())
        if match:
            sections[match[1]] = int(match[2])
    return {
        "bytes": path.stat().st_size,
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "sections": sections,
    }


def main():
    """Write auditable measurements before enforcing the optional size limit."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path, help="staged directory containing the executable and shared libraries")
    parser.add_argument("--executable", required=True, help="executable filename within that directory")
    parser.add_argument("--output", required=True, type=Path, help="JSON record to preserve across cargo clean")
    parser.add_argument("--baseline", type=Path, help="previous JSON from the same build configuration")
    parser.add_argument("--max-growth", type=int, default=0, help="allowed total byte growth when comparing (default: zero)")
    args = parser.parse_args()
    if Path(args.executable).name != args.executable or not (args.directory / args.executable).is_file():
        parser.error("--executable must name an existing file directly inside the staged directory")
    paths = sorted(path for path in args.directory.iterdir() if path.is_file() and (path.suffix == ".so" or path.name == args.executable))
    files = {path.name: capture_file(path) for path in paths}
    record = {
        "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "working_tree": subprocess.check_output(["git", "status", "--porcelain"], text=True).splitlines(),
        "rustc": subprocess.check_output(["rustc", "+nightly", "--version"], text=True).strip(),
        "directory": str(args.directory.resolve()),
        "files": files,
        "total_bytes": sum(item["bytes"] for item in files.values()),
    }
    growth = None
    if args.baseline:
        baseline = json.loads(args.baseline.read_text())
        if set(baseline["files"]) != set(files):
            parser.error("baseline and candidate must contain the same artifact filenames")
        if baseline["rustc"] != record["rustc"] or baseline["directory"] != record["directory"]:
            parser.error("baseline and candidate must use the same compiler and staged directory")
        growth = record["total_bytes"] - baseline["total_bytes"]
        record["baseline"] = str(args.baseline)
        record["growth_bytes"] = growth
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
