#!/usr/bin/env python3
"""Verify that historical UI manifests and commits select one exact COM revision.

The checked-in ledger records the initial history. Later commits use their root
dependency pins; revision trailers, when present, must agree. COM manifests are
inspected from Git objects, so archives need no build artifacts or checkout.
"""

import argparse
from dataclasses import dataclass
from pathlib import Path
import re
import subprocess
import tomllib


COM_URL = "ssh://git@github.com/NeoCogi/neocogi-com.git"
COM_REPOSITORY = "git@github.com:NeoCogi/neocogi-com.git"
REVISION = re.compile(r"[0-9a-f]{40}")


@dataclass(frozen=True)
class HistoryLink:
    """One UI commit and its optional exact companion revision."""

    ui: str
    """Full UI commit identifier, in ancestry order."""
    com: str | None
    """Full COM commit identifier; absent before the first COM dependency."""


def git(repository, *arguments, input=None):
    """Read repository objects without changing refs, indexes or worktrees."""
    return subprocess.check_output(
        ["git", "-C", str(repository), *arguments], input=input,
        stderr=subprocess.PIPE,
    )


def read_ledger(text):
    """Reject malformed, duplicate or abbreviated identifiers before Git access."""
    lines = text.splitlines()
    if not lines or lines[0] != "ui_commit\tcom_commit":
        raise ValueError("COM history requires the ui_commit/com_commit TSV header")
    result = []
    seen = set()
    for number, line in enumerate(lines[1:], start=2):
        fields = line.split("\t")
        if len(fields) != 2:
            raise ValueError(f"COM history line {number}: expected two fields")
        ui, com = fields
        if not REVISION.fullmatch(ui) or (com != "-" and not REVISION.fullmatch(com)):
            raise ValueError(f"COM history line {number}: full commit identifiers required")
        if ui in seen:
            raise ValueError(f"COM history line {number}: duplicate UI commit")
        seen.add(ui)
        result.append(HistoryLink(ui, None if com == "-" else com))
    if not result:
        raise ValueError("COM history ledger is empty")
    return result


def sdk_package(name):
    """Recognize the historical SDK/compiler names without including UI tooling."""
    roots = ("nc-com", "neocogi-com", "neocogi-idl")
    return any(name == root or name.startswith(root + "-") for root in roots) or name == "neocogi-native-runtime-link"


def dependency_tables(manifest):
    """Yield ordinary, workspace, conditional and patched Cargo dependencies."""
    kinds = ("dependencies", "dev-dependencies", "build-dependencies")
    for kind in kinds:
        yield manifest.get(kind, {})
    yield manifest.get("workspace", {}).get("dependencies", {})
    for target in manifest.get("target", {}).values():
        for kind in kinds:
            yield target.get(kind, {})
    yield from manifest.get("patch", {}).values()


def manifest_links(manifest, expected):
    """Validate source selection and return the actual SDK packages requested.

    Workspace inheritance leaves the source with its owning root declaration.
    Aliases use the explicit package name; feature, artifact and version policy
    stay independent of this source check.
    """
    requested = set()
    for table in dependency_tables(manifest):
        for alias, dependency in table.items():
            package = dependency.get("package", alias) if isinstance(dependency, dict) else alias
            if not sdk_package(package):
                continue
            if isinstance(dependency, dict) and dependency.get("workspace") is True:
                continue
            if expected is None:
                raise ValueError(f"{alias}: SDK dependency has no recorded COM revision")
            if not isinstance(dependency, dict):
                raise ValueError(f"{alias}: historical SDK dependency must select Git")
            if dependency.get("git") != COM_URL or dependency.get("rev") != expected:
                raise ValueError(f"{alias}: SDK Git source/revision differs from its commit link")
            if any(key in dependency for key in ("path", "branch", "tag")):
                raise ValueError(f"{alias}: SDK dependency has an alternate or floating source")
            requested.add(package)
    return requested


def message_link(message):
    """Read the unique COM trailer pair, rejecting partial or conflicting links."""
    # Git owns trailer syntax. Examples quoted in an ordinary message body must
    # not become revision declarations just because they contain these keys.
    trailers = subprocess.check_output(
        ["git", "interpret-trailers", "--parse"], input=message.encode(),
        stderr=subprocess.PIPE,
    ).decode()
    revisions = re.findall(r"^NeoCogi-Com-Commit: (.+)$", trailers, flags=re.M | re.I)
    repositories = re.findall(r"^NeoCogi-Com-Repository: (.+)$", trailers, flags=re.M | re.I)
    if not revisions and not repositories:
        return None
    if len(revisions) != 1 or not REVISION.fullmatch(revisions[0]):
        raise ValueError("commit requires exactly one full COM revision trailer")
    if repositories != [COM_REPOSITORY]:
        raise ValueError("commit requires the companion COM repository trailer")
    return revisions[0]


def declared_revision(manifest):
    """Read a later commit's authoritative root pin without requiring a trailer."""
    revisions = set()
    for table in dependency_tables(manifest):
        for alias, dependency in table.items():
            package = dependency.get("package", alias) if isinstance(dependency, dict) else alias
            if not sdk_package(package) or (isinstance(dependency, dict) and dependency.get("workspace") is True):
                continue
            if not isinstance(dependency, dict) or dependency.get("git") != COM_URL:
                raise ValueError(f"{alias}: root SDK declaration must select the companion Git source")
            revision = dependency.get("rev", "")
            if not isinstance(revision, str) or not REVISION.fullmatch(revision):
                raise ValueError(f"{alias}: root SDK declaration must pin a full commit")
            revisions.add(revision)
    if len(revisions) > 1:
        raise ValueError("root SDK declarations select different COM revisions")
    return next(iter(revisions), None)


class ObjectReader:
    """Read each immutable blob at most once during the complete history audit."""

    def __init__(self, repository):
        """Associate the cache with one repository's object database."""
        self.repository = repository
        self.blobs = {}

    def tree(self, revision):
        """Return filename-to-blob mappings from a complete committed snapshot."""
        result = {}
        for entry in git(self.repository, "ls-tree", "-rz", revision).split(b"\0"):
            if entry:
                header, name = entry.split(b"\t", 1)
                result[name.decode()] = header.split()[2].decode()
        return result

    def blob(self, identifier):
        """Cache immutable contents while leaving binary asset bytes untouched."""
        if identifier not in self.blobs:
            self.blobs[identifier] = git(self.repository, "cat-file", "blob", identifier)
        return self.blobs[identifier]


def verify(ui, com, ledger):
    """Check ledger coverage, every manifest source, package existence and progress."""
    links = read_ledger(ledger.read_text())
    commits = git(ui, "rev-list", "--reverse", "HEAD").decode().splitlines()
    if commits[:len(links)] != [link.ui for link in links]:
        raise ValueError("COM ledger must cover the complete initial UI ancestry in order")
    expected = {link.ui: link.com for link in links}
    ui_objects = ObjectReader(ui)
    com_objects = ObjectReader(com)
    packages = {}
    seen_manifest_checks = set()
    seen_com = set()
    previous = None
    linked = 0
    for identifier in commits:
        tree = ui_objects.tree(identifier)
        message = git(ui, "show", "-s", "--format=%B", identifier).decode()
        revision = message_link(message)
        if identifier in expected and revision != expected[identifier]:
            raise ValueError(f"{identifier}: commit trailer disagrees with the COM ledger")
        if identifier not in expected:
            root_revision = declared_revision(tomllib.loads(ui_objects.blob(tree["Cargo.toml"]).decode()))
            if revision is not None and revision != root_revision:
                raise ValueError(f"{identifier}: COM trailer disagrees with the root dependency pin")
            revision = root_revision
        if revision:
            linked += 1
            if revision not in packages:
                # Resolving commit type rejects annotated tags, blobs and other
                # object kinds even if they happen to use a full identifier.
                if git(com, "cat-file", "-t", revision).strip() != b"commit":
                    raise ValueError(f"{revision}: COM link must name a commit")
                package_names = set()
                for path, blob in com_objects.tree(revision).items():
                    if path.endswith("Cargo.toml"):
                        name = tomllib.loads(com_objects.blob(blob).decode()).get("package", {}).get("name")
                        if name:
                            package_names.add(name)
                packages[revision] = package_names
            if identifier in expected and previous and revision != previous:
                git(com, "merge-base", "--is-ancestor", previous, revision)
            previous = revision
            seen_com.add(revision)
        elif previous:
            raise ValueError(f"{identifier}: COM linkage disappears after its introduction")
        for path, blob in tree.items():
            if not path.endswith("Cargo.toml") or (blob, revision) in seen_manifest_checks:
                continue
            manifest = tomllib.loads(ui_objects.blob(blob).decode())
            requested = manifest_links(manifest, revision)
            if revision and not requested <= packages[revision]:
                raise ValueError(f"{identifier}:{path}: package absent from the selected COM revision")
            seen_manifest_checks.add((blob, revision))
    return len(commits), linked, len(seen_com), len(seen_manifest_checks)


def main():
    """Run the read-only audit against this checkout and the companion repository."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ui", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--com", type=Path)
    parser.add_argument("--ledger", type=Path)
    options = parser.parse_args()
    com = options.com or options.ui.parent / "neocogi-com"
    ledger = options.ledger or options.ui / "docs/project/COM_HISTORY.tsv"
    commits, linked, revisions, manifests = verify(options.ui, com, ledger)
    print(f"Verified {commits} UI commits, {linked} COM links, {revisions} COM revisions and {manifests} manifest checks.")


if __name__ == "__main__":
    main()
