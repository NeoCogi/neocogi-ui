# Releasing NeoCogi UI

The coordinated release is **0.1.0-alpha.2**. All workspace packages inherit
one version, and internal production/build dependencies specify the same exact
registry version alongside their development paths. Test-only peers use local
paths without versions so Cargo omits their cyclic registry edges on publication.
Platform dependencies use exact `=0.1.0-alpha.2` registry requirements; COM and
IDL use `=0.1.0`, and `rs-math3d` uses `=0.20.1`. Rebuild hosts and providers together;
pre-release wire compatibility across revisions is not promised.

## Release boundary

The 19 reusable packages are:

- `neocogi-ui-build`, the canonical packaged UI/GFX/Skin IDL source owner.
- `neocogi-gfx-support`, `neocogi-appearance`, `neocogi-gfx-impl`,
  `neocogi-gfx` and `neocogi-gfx-dylib`.
- `neocogi-skin-authoring`, `neocogi-skin-impl`, `neocogi-skin` and
  `neocogi-skin-dylib`.
- `neocogi-ui`, `neocogi-ui-design`, `neocogi-ui-document`,
  `neocogi-ui-support`, `neocogi-ui-nodes`, `neocogi-ui-nodes-dylib`,
  `neocogi-ui-runtime` and `neocogi-ui-runtime-dylib`.
- `neocogi-ui-samples`, reusable generated example contracts used by document tests.

Applications, example implementation plugins, native composition crates, test
harnesses and asset-export executables retain `publish = false`. Publication
policy is checked from Cargo metadata, including dev-dependency closure; a private
application cannot accidentally become part of this release.

`neocogi-ui-build` owns every authored production UI/GFX/Skin `.ncidl` file.
It materializes those embedded sources under each consumer's `OUT_DIR`, combines
the matching packaged Platform sources, and selects explicit imported/emitted
namespaces. Normal builds neither reach outside their package nor copy source
files into the checkout. Sample contracts remain owned by their sample package.

Some archived tests need fonts, images, themes, documentation examples or shared
test tooling. Those inputs are derived into `tests/package-inputs` by the explicit
`scripts/package_inputs.py` preparation command. Their original repository files
remain authoritative; `--check` rejects changed, missing or stale derived bytes.
The same command derives each package's license and attribution notices from
`LICENSE` and `LICENSES`. Do not edit derived copies directly.

## Qualification

Run the ordinary UI, asset, native renderer and browser CI suites from
[the Rust workflow](../../.github/workflows/rust.yml). The archive gate adds
independent evidence that a checkout build cannot provide:

```sh
python3 scripts/package_inputs.py --check
python3 -m unittest discover -s scripts -p test_release_packages.py
rustup component add clippy
python3 scripts/check_packages.py --publish-dry-run --stable-toolchain stable --output /tmp/neocogi-ui-alpha2-archives
```

Use `python3 scripts/package_inputs.py` after changing a canonical package input.
The qualifier invokes the pinned toolchain explicitly, so install its Clippy
component before entering the external consumer workspace. A `cargo +<toolchain>`
override does not install the component list from `rust-toolchain.toml`.
`--allow-dirty` is available during development; final qualification must use a
clean committed tree, including qualification tooling and CI changes. `--offline` is useful after all pinned dependencies are
cached, using package mode without `--publish-dry-run`: Cargo's publisher needs
registry access even when all dependencies are cached. The output directory must
not already exist.

With `--publish-dry-run` the gate uses Cargo's multi-package publisher in its
explicit nonuploading mode to create real `.crate` archives. Without that option
it uses `cargo package`. It then inspects normalized manifests,
rejects path/Git/unversioned dependencies and escaped or linked archive members,
and verifies schema ownership, package READMEs and license notices. It extracts
all selected packages into a temporary workspace outside the checkout. Builds
and tests enable every library feature. The independent acceptance application
emits its own derived event contract, calls real COM methods, activates the GFX,
Skin, UI runtime and node providers, derives a real appearance revision,
processes button input, authenticates rendered blocks and retains an owned event after widget removal. It runs with both linked
providers and separately compiled provider DLLs, checking the full prerelease
version in each descriptor. The optional stable run verifies a consumer without
the development workspace's nightly Cargo settings; it does not establish an MSRV.

Cargo's workspace publisher rejects cycles that pass through dev dependencies.
Repository tests deliberately exercise peers in both directions. Keep those
links path-only; do not publish those cyclic dev-dependency entries. The explicit
input generator derives their exact registry requirements and feature policies
into each owning archive's `tests/package-inputs/dependencies.json`. Freshness
checks keep the original manifest authoritative. This follows Cargo's
[documented omission of versionless dev dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#development-dependencies).

The qualifier first builds every unmodified normalized production archive and
runs the linked/DLL/stable consumers. Only afterward does it restore the archived
test-only declarations to the temporary extracted manifests and execute unit,
integration and documentation tests. Production dependencies and source bytes
are never edited. Thus the release has an acyclic upload graph while all existing
regressions remain executable from the complete archived source set through this
test preparation step. A plain `cargo test` against an individual normalized
archive does not include the omitted peer links.

Before the first upload, unpublished same-version peers are resolved with
explicit temporary registry patches. Packaging uses checkout paths only to
resolve metadata; no code is compiled there. Every internal implementation used
by subsequent compilation, tests and nested fixture consumers comes from an
inspected archive. Temporary Cargo configuration is scoped to the run; the
caller's global configuration is not modified. The production publication graph
excludes cyclic test-only edges; the later
archive-test phase restores them without requiring premature uploads. All Cargo
commands share one target directory sequentially, and internal artifacts are
invalidated before using normalized archive timestamps.

On success the optional output directory contains the exact qualified archives
and `SHA256SUMS`. Keep these with the logs as release evidence. The CI archive
job repeats the same gate on a committed checkout. Native SDL/GPU and browser
execution belong to the separate renderer/browser jobs; archive qualification's
logical renderer does not stand in for those tests.

## Publication

Archive qualification never uploads or changes registry state. After approving
the exact committed release, an authenticated maintainer can publish the selected
packages with Cargo's multi-package publisher. Exclude all private workspace
members, retain exact peer versions, and allow dependencies to become visible in
the registry before dependent uploads. Run `cargo publish --dry-run` first.
Cargo's [publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
documents immutable versions, registry authentication and upload validation.

Record the release commit in the release tag and confirm that registry-only
consumers resolve the published checksums. A local patch qualification establishes
archive completeness and interoperability; registry ownership, server acceptance,
index visibility and registry-only resolution are checked at upload time.
