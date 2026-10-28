# Source history and standalone extraction

NeoCogi UI continues the microui-redux UI lineage through
[NeoCogi UI](https://github.com/NeoCogi/neocogi-ui). The extraction uses
UI branch `refactor/external-neocogi-com` at
`84d8fdd5`.

The imported ancestry starts with the original November 21, 2022 commit
`6db90618220a0b462999d24a419825978566919d`. It includes the microui-redux
`0.9-alpha.0` baseline at
`6b4fd86f`,
which maps to `b39cfe076ff5140b3d14371057004c69720cc7a2` here. The local
`microui-redux-0.9-alpha.0` tag marks that imported baseline.

The filtered import contains **1,411 commits**. Original authors, committers,
timestamps, messages and retained source changes are preserved. Removing files
and commits changes downstream Git identities. The original-to-imported mapping
is retained locally in `.git/info/ui-commit-map`.

## Extraction boundaries

Retained source includes the UI client API, runtime, nodes, support, shared IDL,
runtime documents, serialization/manifests, renderer/platform providers, GFX,
skin, appearance, assets, themes, UI examples and their build/validation helpers.
Their historical names and file moves remain in the imported ancestry.
`neocogi-ui-design` is a library for document/settings serialization and extension
manifests used by the runtime loader and extension example.

Designer and theme-editor applications, their tests/scripts, in-tree COM/IDL
implementations and SDK-only tools/examples are excluded from every imported
revision. Review, audit, plan, proposal and experiment documents are excluded
throughout history. Documentation-only commits recording those activities are
also omitted; implementation fixes and their tests remain.

The import follows the standalone COM extraction pattern: filtered source
history first, followed by explicit workspace and documentation commits.
Runnable UI applications and shared example hosts/assets/providers live under
`examples/`; libraries remain under their subsystem in `crates/`, with `tools/`,
`scripts/` and `docs/` for their existing responsibilities.
The source checkouts are unchanged by this extraction.

## COM source

The SDK, host and compiler are Git dependencies from
`git@github.com:NeoCogi/neocogi-com.git`, pinned together to
[`4d8d7228`](https://github.com/NeoCogi/neocogi-com/commit/4d8d7228efbcc400f63a016f9d04ee81f6f5baf8),
the sibling COM checkout's `master`/`origin/master` at extraction. Cargo uses the
SSH URL form of the same repository and commits its complete lockfile.
Upstream documentation links use that revision. Follow the
[COM dependency guide](../guides/COM_DEPENDENCY.md) when advancing the pin.
