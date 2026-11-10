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
    paths = list((root / "crates").glob("*/Cargo.toml"))
    return sorted(path.parent for path in paths if tomllib.loads(path.read_text())["package"].get("publish") == ["crates-io"])


def fixture_sources(root=ROOT):
    """One fixture tree per public package; all copies derive from authored owners."""
    common = {"com_dependencies.rs": root / "scripts/com_dependencies.rs", "workspace.toml": root / "Cargo.toml"}
    sdk = dict(common)
    sdk.update({"assets/" + p.name: p for p in (root / "assets").iterdir() if p.suffix in {".png", ".ttf"}})
    sdk.update({"themes/" + str(p.relative_to(root / "themes")): p for p in (root / "themes").rglob("*") if p.is_file()})
    sdk.update({name: root / "examples/assets/src" / name for name in ["recipe.rs", "recipe_data.rs"]})
    sdk.update({"button_events.rs": root / "crates/ui-contracts/docs/button_events.rs",
                "document.md": root / "crates/ui/docs/document.md", "application.md": root / "README.md"})
    sdk.update({"document/" + p.name: p for p in (root / "crates/ui/tests/document/fixtures").iterdir() if p.is_file()})
    sdk.update({"sample/abi/" + str(p.relative_to(root / "examples/ui-samples/src/abi")): p
                for p in (root / "examples/ui-samples/src/abi").rglob("*") if p.is_file()})
    sdk["sample/schemas/neocogi/demo.ncidl"] = root / "examples/ui-samples/schemas/neocogi/demo.ncidl"
    return {root / relative / "tests/package-inputs": {name: p.read_bytes() for name, p in files.items()}
            for relative, files in [("crates/ui-contracts", common), ("crates/ui", sdk)]}


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
