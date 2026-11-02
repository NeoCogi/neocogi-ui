#!/usr/bin/env python3
"""Verify exact COM sources across historical Git pins and published releases.

The checked-in ledger records the initial history. Later commits use their root
dependency pins; revision trailers, when present, must agree with Git selections.
Historical package availability is checked from Git objects. Cargo resolves
registry releases and verifies their checksums; this audit does not fetch them.
"""

import argparse
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
import posixpath
import re
import subprocess
import tomllib


COM_URL = "ssh://git@github.com/NeoCogi/neocogi-com.git"
COM_REPOSITORY = "git@github.com:NeoCogi/neocogi-com.git"
HISTORICAL_BASELINE = "72e5b4481fef861777ffc1f42e1153ce8d2e99e4"
"""Independent end of the complete initial ledger; never inferred from ledger input."""
REVISION = re.compile(r"[0-9a-f]{40}")
EXACT_VERSION = re.compile(r"=[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?")


@dataclass(frozen=True)
class SdkSource:
    """One source identity shared by a snapshot's SDK and compiler declarations."""

    kind: str
    """Either git for historical commits or crates.io for published packages."""
    pin: str
    """Full Git commit ID or exact Cargo version requirement including its equals sign."""


def dependency_source(alias, dependency):
    """Reject floating, alternate or mixed SDK sources before comparing identities."""
    declaration = {"version": dependency} if isinstance(dependency, str) else dependency
    if not isinstance(declaration, dict) or any(key in declaration for key in ("path", "branch", "tag", "registry", "registry-index")):
        raise ValueError(f"{alias}: SDK dependency has an alternate or floating source")
    if "git" in declaration:
        revision = declaration.get("rev", "")
        if declaration["git"] != COM_URL or not isinstance(revision, str) or not REVISION.fullmatch(revision):
            raise ValueError(f"{alias}: SDK Git source must pin a full companion commit")
        return SdkSource("git", revision)
    version = declaration.get("version", "")
    if "rev" in declaration or not isinstance(version, str) or not EXACT_VERSION.fullmatch(version):
        raise ValueError(f"{alias}: SDK registry dependency must pin one exact crates.io version")
    return SdkSource("crates.io", version)


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


def manifest_links(manifest, expected, workspace=None):
    """Validate source selection and return the actual SDK packages requested.

    Workspace inheritance leaves the source with its owning root declaration.
    Aliases use the explicit package name; feature, artifact and version policy
    stay independent of this source check.
    """
    requested = set()
    workspace = manifest.get("workspace", {}).get("dependencies", {}) if workspace is None else workspace
    for table in dependency_tables(manifest):
        for alias, dependency in table.items():
            if isinstance(dependency, dict) and dependency.get("workspace") is True:
                # Resolve before classifying: an arbitrary alias may own an SDK package.
                if alias not in workspace:
                    raise ValueError(f"{alias}: inherited dependency has no owning workspace declaration")
                if any(key in dependency for key in ("package", "git", "path", "rev", "version", "branch", "tag", "registry", "registry-index")):
                    raise ValueError(f"{alias}: inherited dependency cannot override its workspace source")
                dependency = workspace[alias]
                if isinstance(dependency, dict) and dependency.get("workspace") is True:
                    raise ValueError(f"{alias}: workspace declaration cannot itself inherit")
            package = dependency.get("package", alias) if isinstance(dependency, dict) else alias
            if not sdk_package(package):
                continue
            if expected is None:
                raise ValueError(f"{alias}: SDK dependency has no recorded source")
            if dependency_source(alias, dependency) != expected:
                raise ValueError(f"{alias}: SDK source/pin differs from its snapshot selection")
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


def declared_source(manifest):
    """Read a later commit's authoritative root pin without requiring a trailer."""
    sources = set()
    for table in dependency_tables(manifest):
        for alias, dependency in table.items():
            package = dependency.get("package", alias) if isinstance(dependency, dict) else alias
            if not sdk_package(package) or (isinstance(dependency, dict) and dependency.get("workspace") is True):
                continue
            sources.add(dependency_source(alias, dependency))
    if len(sources) > 1:
        raise ValueError("root SDK declarations select different COM sources or pins")
    return next(iter(sources), None)


class ObjectReader:
    """Read each immutable blob at most once during the complete history audit."""

    def __init__(self, repository):
        """Associate the cache with one repository's object database."""
        self.repository = repository
        self.blobs = {}
        self.manifests = {}

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

    def manifest(self, identifier):
        """Cache syntax only; resolved inheritance is checked with each owning-root context."""
        if identifier not in self.manifests:
            self.manifests[identifier] = tomllib.loads(self.blob(identifier).decode())
        return self.manifests[identifier]


def workspace_context(path, tree, reader):
    """Resolve only Cargo workspace inheritance, including explicit/nested workspace roots.

    This does not resolve features, versions or local package graphs. The root blob
    identifies the complete context needed by an inherited SDK source check.
    """
    parent = PurePosixPath(path).parent
    explicit = reader.manifest(tree[path]).get("package", {}).get("workspace")
    if explicit is not None:
        candidate = posixpath.normpath(str(parent / explicit / "Cargo.toml"))
        if candidate.startswith(("../", "/")) or candidate not in tree:
            raise ValueError(f"{path}: explicit owning workspace is absent from its snapshot")
        candidates = [candidate]
    else:
        candidates = [str(directory / "Cargo.toml") for directory in [parent, *parent.parents]]
    for candidate in candidates:
        blob = tree.get(candidate)
        if blob is not None:
            manifest = reader.manifest(blob)
            if "workspace" in manifest:
                return manifest["workspace"].get("dependencies", {}), blob
    if explicit is not None:
        raise ValueError(f"{path}: explicit owner has no workspace declaration")
    return {}, None


def verify(ui, com, ledger, *, baseline=HISTORICAL_BASELINE):
    """Check ledger coverage, every manifest source, package existence and progress."""
    links = read_ledger(ledger.read_text())
    try:
        if git(ui, "cat-file", "-t", baseline).strip() != b"commit":
            raise ValueError("history baseline must name a commit")
        git(ui, "merge-base", "--is-ancestor", baseline, "HEAD")
    except subprocess.CalledProcessError as error:
        raise ValueError("history baseline must exist and be an ancestor of HEAD") from error
    anchored = git(ui, "rev-list", "--reverse", baseline).decode().splitlines()
    if anchored != [link.ui for link in links] or links[-1].ui != baseline:
        raise ValueError("COM ledger must cover the complete initial UI ancestry through the fixed baseline")
    # Only this independently anchored initial chain promises monotonic companion progress.
    previous = None
    for link in links:
        if link.com is None and previous is not None:
            raise ValueError("COM linkage disappears inside the initial ledger")
        if link.com is not None:
            if previous is not None and previous != link.com:
                git(com, "merge-base", "--is-ancestor", previous, link.com)
            previous = link.com
    introduced = next((link.ui for link in links if link.com), None)
    required = set(git(ui, "rev-list", "--ancestry-path", f"{introduced}..HEAD").decode().splitlines()) if introduced else set()
    if introduced:
        required.add(introduced)
    commits = git(ui, "rev-list", "--reverse", "HEAD").decode().splitlines()
    expected = {link.ui: link.com for link in links}
    ui_objects = ObjectReader(ui)
    com_objects = ObjectReader(com)
    packages = {}
    seen_manifest_checks = set()
    seen_com = set()
    linked = 0
    for identifier in commits:
        tree = ui_objects.tree(identifier)
        message = git(ui, "show", "-s", "--format=%B", identifier).decode()
        trailer = message_link(message)
        if identifier in expected and trailer != expected[identifier]:
            raise ValueError(f"{identifier}: commit trailer disagrees with the COM ledger")
        source = SdkSource("git", trailer) if trailer else None
        if identifier not in expected:
            root_source = declared_source(ui_objects.manifest(tree["Cargo.toml"])) if "Cargo.toml" in tree else None
            if trailer is not None and source != root_source:
                raise ValueError(f"{identifier}: COM trailer disagrees with the root dependency pin")
            source = root_source
        if source:
            linked += 1
        if source and source.kind == "git":
            revision = source.pin
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
            seen_com.add(revision)
        elif source is None and identifier in required:
            raise ValueError(f"{identifier}: COM linkage disappears in a descendant of its introduction")
        for path, blob in tree.items():
            if not path.endswith("Cargo.toml"):
                continue
            workspace, context = workspace_context(path, tree, ui_objects)
            key = (blob, source, context)
            if key in seen_manifest_checks:
                continue
            requested = manifest_links(ui_objects.manifest(blob), source, workspace)
            if source and source.kind == "git" and not requested <= packages[source.pin]:
                raise ValueError(f"{identifier}:{path}: package absent from the selected COM revision")
            seen_manifest_checks.add(key)
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
    print(f"Verified {commits} UI commits, {linked} SDK-dependent snapshots, {revisions} historical COM revisions and {manifests} manifest checks.")


if __name__ == "__main__":
    main()
