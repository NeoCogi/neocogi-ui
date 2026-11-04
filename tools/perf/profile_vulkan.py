#!/usr/bin/env python3
"""Measure warmed full-frame Vulkan intervals, or generate owned-dependency overrides.

Only explicitly launched executables are observed. Raw logs, CPU accounting and
Vulkan synchronization durations accompany every accepted measurement.
"""

import argparse
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import sys
import tomllib


def write_overrides(destination: Path) -> None:
    """Resolve publishable sibling packages without changing any repository manifest."""
    root = Path(__file__).resolve().parents[2]
    platform = root.parent / "neocogi-platform"
    math = root.parent / "rs-math3d"
    manifest = tomllib.loads((platform / "Cargo.toml").read_text())
    assert (math / "Cargo.toml").is_file(), "rs-math3d sibling checkout is required"
    packages = {"rs-math3d": math}
    for member in manifest["workspace"]["members"]:
        path = platform / member
        package = tomllib.loads((path / "Cargo.toml").read_text())["package"]
        if package.get("publish", True) is not False:
            packages[package["name"]] = path
    with destination.open("x") as output:
        output.write("[patch.crates-io]\n")
        for name, path in packages.items():
            output.write(f"{json.dumps(name)} = {{ path = {json.dumps(str(path))} }}\n")
    print(destination)


def measure(executables, directory: Path, frames: int, warmup: int, runs: int) -> None:
    """Alternate process order and retain all runs, including slow presentation intervals."""
    directory.mkdir(parents=True, exist_ok=False)
    library = directory / "present-clock.so"
    # Link the loader explicitly: plugins load their native dependencies into a local scope,
    # while the preload observer resolves the real entry points through RTLD_NEXT.
    subprocess.run([
        "cc", "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC", "-O2",
        str(Path(__file__).with_name("present_clock.c")), "-ldl", "-Wl,--no-as-needed",
        "-lvulkan", "-o", str(library),
    ], check=True)
    rows = []
    for repetition in range(runs):
        sequence = executables if repetition % 2 == 0 else list(reversed(executables))
        for label, executable in sequence:
            log = directory / f"{repetition}-{label}.log"
            counters = directory / f"{repetition}-{label}-cpu.txt"
            environment = dict(os.environ, LC_ALL="C", NEOCOGI_DEMO_FRAMES=str(frames), NEOCOGI_PROFILE_WARMUP=str(warmup))
            environment["LD_PRELOAD"] = str(library) + (":" + environment["LD_PRELOAD"] if environment.get("LD_PRELOAD") else "")
            with log.open("w") as output:
                subprocess.run([
                    "perf", "stat", "--no-big-num", "-e", "task-clock", "-o", str(counters),
                    "--", str(executable),
                ], cwd=executable.parent, env=environment, stdout=output, stderr=subprocess.STDOUT, check=True)
            lines = log.read_text().splitlines()
            observations = [json.loads(line.removeprefix("NEOCOGI_PRESENT_CLOCK ")) for line in lines if line.startswith("NEOCOGI_PRESENT_CLOCK ")]
            if len(observations) != 1:
                raise RuntimeError(f"expected one presentation measurement; inspect {log}")
            observation = observations[0]
            if observation["presentations"] != frames or observation["failures"] or observation["intervals"] != frames - warmup - 1:
                raise RuntimeError(f"incomplete or failed presentation run; inspect {log}")
            counter = re.search(r"([\d.]+)\s+msec task-clock", counters.read_text())
            if counter is None:
                raise RuntimeError(f"task-clock was unavailable; inspect {counters}")
            stages = [json.loads(line.removeprefix("NEOCOGI_VULKAN_STAGE ")) for line in lines if line.startswith("NEOCOGI_VULKAN_STAGE ")]
            row = dict(label=label, executable=str(executable), repetition=repetition, **observation,
                       full_process_cpu_ms_per_frame=float(counter[1]) / frames, stages=stages)
            rows.append(row)
            (directory / "runs.json").write_text(json.dumps(rows, indent=2) + "\n")
            print(f"{label} run {repetition + 1}: {observation['fps']:.1f} warmed FPS, {row['full_process_cpu_ms_per_frame']:.3f} CPU ms/frame", flush=True)
    summaries = {}
    for label, _ in executables:
        group = [row for row in rows if row["label"] == label]
        summaries[label] = {
            "median_fps": statistics.median(row["fps"] for row in group),
            "fps_range": [min(row["fps"] for row in group), max(row["fps"] for row in group)],
            "median_full_process_cpu_ms_per_frame": statistics.median(row["full_process_cpu_ms_per_frame"] for row in group),
        }
    (directory / "summary.json").write_text(json.dumps(summaries, indent=2) + "\n")
    print(json.dumps(summaries, indent=2))


def main() -> None:
    """Keep dependency selection and benchmark execution explicit and independently usable."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--local-overrides", type=Path, help="write a new Cargo config for the sibling Platform and math checkouts")
    parser.add_argument("--exe", action="append", default=[], metavar="LABEL=PATH", help="built demo executable; repeat to compare builds")
    parser.add_argument("--output", type=Path, help="new directory for raw logs and summaries")
    parser.add_argument("--frames", type=int, default=15000, help="total frames per process (default: 15000)")
    parser.add_argument("--warmup", type=int, default=1000, help="presentations before timing starts (default: 1000)")
    parser.add_argument("--runs", type=int, default=3, help="repetitions per executable (default: 3)")
    args = parser.parse_args()
    if args.local_overrides:
        if args.exe or args.output:
            parser.error("use --local-overrides independently of benchmark arguments")
        write_overrides(args.local_overrides.resolve())
        return
    if sys.platform != "linux":
        parser.error("the preload frame clock requires Linux")
    if not args.exe or args.output is None:
        parser.error("provide --exe LABEL=PATH and --output DIRECTORY")
    if not 0 <= args.warmup < args.frames - 1 or not 0 < args.frames <= 0xFFFF_FFFF or args.runs < 1:
        parser.error("require positive runs and warmup + 1 < frames <= u32::MAX")
    executables = []
    labels = set()
    for item in args.exe:
        label, separator, path = item.partition("=")
        if not separator or re.fullmatch(r"[A-Za-z0-9_-]+", label) is None or label in labels:
            parser.error("use unique labels containing letters, digits, hyphens or underscores")
        labels.add(label)
        executables.append((label, Path(path).resolve(strict=True)))
    measure(executables, args.output.resolve(), args.frames, args.warmup, args.runs)


if __name__ == "__main__":
    main()
