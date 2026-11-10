# UI and COM history

`neocogi-ui` and [`neocogi-com`](https://github.com/NeoCogi/neocogi-com) evolve
through corresponding revisions. Each historical UI commit that uses COM pins
its SDK/compiler dependencies to the matching commit in
`git@github.com:NeoCogi/neocogi-com.git`.

The [revision ledger](COM_HISTORY.tsv) covers the complete **1,395-commit** ancestry
through independently fixed baseline `72e5b4481fef861777ffc1f42e1153ce8d2e99e4`.
The auditor requires that baseline to exist as a commit and remain an ancestor of
HEAD; its coverage boundary cannot be inferred from an editable ledger tail.
It records **568 UI-to-COM links across 268 COM revisions**. A `-` marks each of
the 827 early UI commits before COM was introduced. Historical package names,
versions, features, default-feature selections and artifact policies are retained;
SDK dependency paths become exact Git sources. Workspace selections include
only the packages present in the UI repository.

## Following a revision

A linked commit records its companion revision directly:

```text
NeoCogi-Com-Repository: git@github.com:NeoCogi/neocogi-com.git
NeoCogi-Com-Commit: <full COM commit ID>
```

Read those trailers with `git show --format=fuller <UI revision>` or look up the
UI commit in the ledger. Authors, committers and timestamps are preserved.
Commit IDs change when dependency declarations or parent commits change.
The `microui-redux-0.9-alpha.0` tag selects the corresponding UI baseline.

Later commits use their root Cargo dependency pins as the authority. A COM Git
trailer, when present, must match a Git dependency selection. The current SDK,
host and compiler use the exact published crates.io release
[`0.1.0`](https://crates.io/crates/neocogi-com/0.1.0).
The [dependency guide](../guides/COM_DEPENDENCY.md) explains coordinated updates.

## Verification

With both repositories checked out as siblings, run:

```sh
python3 scripts/check_com_history.py
```

The read-only audit checks exact initial coverage/trailers and monotonic companion
ancestry on that anchored chain. Later snapshots validate their own root selection;
a global traversal neighbor is not an ancestor. Pre-SDK side branches can merge
without inheriting another branch's source, while descendants of SDK introduction
must retain a declared selection. Explicit later SDK rollbacks remain valid.

Inherited aliases resolve through their owning workspace before SDK classification,
including nested/explicit workspaces and target/build/artifact edges. Syntax caches
are context-free; resolved checks include the owning-root blob, so unchanged child
manifests cannot hide removed or changed root declarations. Reported manifest counts
are unique (manifest blob, SDK source, owning-workspace blob) checks, not file counts. Historical Git selections also check
requested package existence; registry selections require one exact crates.io
version across SDK/compiler declarations, with package availability and checksums
validated by Cargo. The audit reads Git objects directly without building archived
snapshots or requiring their packages in the current workspace.
Use `--com path/to/neocogi-com` for another
checkout location.

Older COM revisions predate its standalone workspace manifest. Exact historical
linkage and matching SDK contents are verified; independent buildability of every
archived UI/COM pair is not claimed. Current UI builds use the packaged COM revision
and the committed lockfile.

## Platform companions

[NeoCogi Platform](https://github.com/NeoCogi/neocogi-platform) supplies the
platform and generic graphics contracts. Historical Git-based UI revisions pin
their exact platform commit in Cargo dependencies and a `NeoCogi-Platform-Commit`
trailer. Those snapshots retain their `NeoCogi-Revision-ID` correlation identifiers
and reciprocal companion links in `refs/notes/neocogi-companions`.

The current UI consumes the three published Platform crates at exactly `0.2.0-alpha.2`.
UI itself now has contracts/static/dylib packages; see the three-crate migration report.
Its root manifests and registry checksums in `Cargo.lock` control that selection;
a Platform Git companion trailer is not required for a registry release. See the
[dependency guide](../guides/COM_DEPENDENCY.md) for source ownership and updates.

```sh
git fetch origin refs/notes/neocogi-companions:refs/notes/neocogi-companions
git log --show-notes=neocogi-companions
```

Use the Platform source selected by the UI revision, including its dependency
versions. Historical Git pins still select their recorded commits; registry pins
select the exact published release. Notes provide navigation and never override
Cargo's selected source. Backend-only changes may leave a UI source snapshot unchanged.
