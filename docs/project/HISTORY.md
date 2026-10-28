# UI and COM history

`neocogi-ui` and [`neocogi-com`](https://github.com/NeoCogi/neocogi-com) evolve
through corresponding revisions. Each historical UI commit that uses COM pins
its SDK/compiler dependencies to the matching commit in
`git@github.com:NeoCogi/neocogi-com.git`.

The [revision ledger](COM_HISTORY.tsv) covers the initial **1,413 UI commits**.
It records **570 UI-to-COM links across 268 COM revisions**. A `-` marks each of
the 843 early UI commits before COM was introduced. Historical package names,
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

Later commits use their root Cargo dependency pins as the authority. If a COM
trailer is present, it must match those declarations. The current SDK, host and
compiler use [`4d8d7228`](https://github.com/NeoCogi/neocogi-com/commit/4d8d7228efbcc400f63a016f9d04ee81f6f5baf8).
The [dependency guide](../guides/COM_DEPENDENCY.md) explains coordinated updates.

## Verification

With both repositories checked out as siblings, run:

```sh
python3 scripts/check_com_history.py
```

The read-only audit checks ledger coverage, commit trailers, every historical
Cargo dependency source, requested package existence and COM ancestry. It reads
Git objects directly; it does not build archived snapshots or require their
packages in the current workspace. Use `--com path/to/neocogi-com` for another
checkout location.

Older COM revisions predate its standalone workspace manifest. Exact historical
linkage and matching SDK contents are verified; independent buildability of every
archived UI/COM pair is not claimed. Current UI builds use the packaged COM revision
and the committed lockfile.

The history retains the UI libraries, renderer/platform providers, resources,
themes and UI examples. Standalone editor applications, reviews and plans remain
outside this repository's history.
