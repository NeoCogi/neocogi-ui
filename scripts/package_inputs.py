#!/usr/bin/env python3
"""Derive package-owned notices and test inputs from their canonical repository sources.

Normal builds never copy or modify source files. This explicit preparation step
keeps checked-in archive inputs reproducible, and --check rejects stale or extra
derived files before package qualification. Published production code never reads
these fixtures; archived tests can use them without a neighboring checkout.
"""
import argparse
import json
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def release_directories(root=ROOT):
    """Select reusable libraries/providers explicitly enabled for crates.io."""
    paths = list((root / "crates").rglob("Cargo.toml")) + [root / "examples/ui-samples/Cargo.toml"]
    return sorted(path.parent for path in paths if tomllib.loads(path.read_text())["package"].get("publish") == ["crates-io"])


def fixture_sources(root=ROOT):
    """Derive fixture bytes and exact test-only peer links from their authoritative inputs."""
    result = {}
    for relative in ["crates/gfx/impl", "crates/skin/impl", "crates/skin/authoring"]:
        destination = root / relative / "tests/package-inputs"
        files = {"assets/" + source.name: source for source in (root / "assets").iterdir() if source.suffix in {".png", ".ttf"}}
        if relative == "crates/gfx/impl":
            files.update({name: root / "examples/assets/src" / name for name in ["recipe.rs", "recipe_data.rs"]})
        else:
            files.update({"themes/" + str(source.relative_to(root / "themes")): source for source in (root / "themes").rglob("*") if source.is_file()})
        result[destination] = files
    for relative in ["crates/ui/api", "crates/ui/runtime"]:
        files = {"com_dependencies.rs": root / "scripts/com_dependencies.rs", "workspace.toml": root / "Cargo.toml"}
        if relative == "crates/ui/runtime":
            files.update({"button_events.rs": root / "crates/ui/api/docs/button_events.rs", "document.md": root / "crates/ui/document/README.md", "application.md": root / "README.md"})
            files.update({"document/" + source.name: source for source in (root / "crates/ui/document/tests/fixtures").iterdir() if source.is_file()})
        result[root / relative / "tests/package-inputs"] = files
    result = {directory: {name: source.read_bytes() for name, source in files.items()} for directory, files in result.items()}
    for directory in release_directories(root):
        manifest = tomllib.loads((directory / "Cargo.toml").read_text())
        peers = {}
        for name, dependency in manifest.get("dev-dependencies", {}).items():
            if not isinstance(dependency, dict) or "path" not in dependency or "version" in dependency:
                continue
            version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
            peers[name] = {key: value for key, value in dependency.items() if key not in {"path", "workspace"}}
            peers[name]["version"] = "=" + version
        if peers:
            result.setdefault(directory / "tests/package-inputs", {})["dependencies.json"] = (
                json.dumps({"schema": 1, "package": manifest["package"]["name"], "dev-dependencies": peers}, indent=2, sort_keys=True) + "\n"
            ).encode()
    return result


def reconcile(*, check, root=ROOT):
    """Repair or check only derived outputs, pruning stale members inside owned fixture roots."""
    differences = []
    notices = (root / "LICENSE").read_text()
    for source in sorted((root / "LICENSES").iterdir()):
        if source.is_file():
            notices += "\n\n----- " + source.name + " -----\n\n" + source.read_text()
    for directory in release_directories(root):
        destination = directory / "LICENSE"
        if not destination.exists() or destination.read_text() != notices:
            differences.append(str(destination.relative_to(root)))
            if not check: destination.write_text(notices)
    for directory, sources in fixture_sources(root).items():
        for source in directory.rglob("*") if directory.exists() else []:
            if source.is_file() and str(source.relative_to(directory)) not in sources:
                differences.append(str(source.relative_to(root)))
                if not check: source.unlink()
        for name, expected in sources.items():
            destination = directory / name
            if not destination.exists() or destination.read_bytes() != expected:
                differences.append(str(destination.relative_to(root)))
                if not check:
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    destination.write_bytes(expected)
    if check and differences:
        raise ValueError("stale derived release inputs; run python3 scripts/package_inputs.py:\n" + "\n".join(differences))
    return differences


def main():
    """Expose explicit generation and a read-only CI freshness check."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    changed = reconcile(check=options.check)
    print("Release inputs current" if options.check else f"Updated {len(changed)} derived release inputs")


if __name__ == "__main__":
    main()
