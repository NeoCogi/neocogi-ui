# Releasing NeoCogi UI

The coordinated UI release is **0.2.0**. Exactly three reusable packages
are published: `neocogi-ui-contracts`, `neocogi-ui` and `neocogi-ui-dylib`.
All workspace packages inherit one version. Every internal production, build and
test dependency has an exact matching registry version alongside its development
path. Platform uses `=0.2.0`, COM/IDL `=0.1.0`, and rs-math3d `=0.21.0`.
Rebuild hosts and providers together using the same exact release. The stable
release retains the existing COM ABI-v1 baseline and UI schema versions.

## Release boundary

Contracts owns the 32 canonical production UI/GFX/Skin/library `.ncidl` sources,
portable values, typed clients and optional `build-support` API. It materializes
packaged sources under each consumer's `OUT_DIR`, composes the matching Platform
source graph, and selects explicit imported/emitted namespaces. The static SDK
owns concrete implementation modules, private runtime/test schemas and
package-contained regression tests. The thin dylib exports the SDK's single
`neocogi.ui` descriptor as `nc_ui`.

Examples, the independent extension and its dylib, sample contracts, native
artifact tooling, asset tools and integration harnesses remain unpublished.
Sample ABI/schema fixtures are derived into the SDK archive for its document
regressions; they are not a fourth production schema owner. Retired crate names,
separate provider DLLs and plugin identities have no compatibility wrappers.
See [features](../guides/FEATURES.md) for ownership and service selection.

The archive gate rejects any release set other than the exact three packages,
private-package dependencies and floating internal versions. Tests require no
restored manifests or unpublished test packages. Fonts, images, themes, shared
test tooling and executed documentation examples derive into
`tests/package-inputs` using `scripts/package_inputs.py`. That command also derives
license notices from `LICENSE` and `LICENSES`. Authoritative originals remain in
their repository locations; do not edit generated copies.

## Qualification

Run the ordinary native/browser CI suites plus the archive gate:

```sh
python3 scripts/package_inputs.py
python3 scripts/package_inputs.py --check
python3 -m unittest discover -s scripts -p test_release_packages.py
rustup component add clippy
python3 scripts/check_packages.py --publish-dry-run --stable-toolchain stable --output /tmp/neocogi-ui-archives
```

The qualifier selects the pinned toolchain explicitly. Install Clippy for that
toolchain before entering the external workspace. `--allow-dirty` permits
iterative development; final release evidence requires a clean committed tree,
including scripts and CI. `--offline` uses `cargo package` after dependencies are
cached; it cannot combine with `--publish-dry-run`, which needs registry access.
No mode uploads packages. The output directory must not already exist.

The gate creates real archives, checks safe regular members, exact schema
ownership, normalized registry dependencies, README and license presence, and
extracts all three packages into a temporary workspace outside the checkout.
Temporary source patches resolve same-version unpublished packages. Packaging
uses checkout paths only for metadata resolution; subsequent compilation and
nested test consumers use exclusively extracted source.

An independent acceptance application generates its own derived event contract,
activates the one UI root, acquires all four typed services, derives appearance,
processes button input, authenticates actual composed blocks and reads an owned
event after widget removal. It runs against linked implementations and the real
archived UI shared library, checking the complete descriptor version. Clippy and
all-feature unit, integration and documentation tests execute from extracted
packages. Optional stable qualification runs the linked consumer without the
workspace's nightly settings; this does not establish an MSRV.

One fresh target under the temporary workspace is shared by archive tests and
their generated Cargo children. Keeping it outside the checkout prevents
trybuild from inheriting development `build-std` settings. Registry/Git caches
are shared through a temporary Cargo home whose configuration contains only
archive patches. All temporary compiled output is removed after the gate.
On success, optional retained output contains the exact three archives and
`SHA256SUMS`. GPU/SDL and actual browser execution are separate CI boundaries;
the archive consumer's logical renderer does not replace them.

When `/tmp` is RAM-backed, set `TMPDIR` to an existing disk-backed scratch
directory outside the checkout. `CARGO_BUILD_JOBS=1` bounds concurrent compilation;
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` and `CARGO_INCREMENTAL=0`
reduce temporary build storage. These settings propagate to nested test consumers.

## Publication

Qualification does not change registry state. Before publishing, require successful
`Rust` and `WebGL demo` push workflows for the exact release commit on `master`,
including every native/backend, macOS/Windows, browser, feature and archive job.
The annotated release tag must identify that validated commit. Record those
workflow/job results outside Cargo targets alongside the archive receipts.

The macOS native jobs use `macos-15-intel` after hosted ARM64 capacity prevented
jobs from starting. Native execution is qualified on x86_64; a separate ARM64
monolithic consumer check retains compilation coverage without linking the
runner's Intel Homebrew SDL library. This does not establish ARM64 native execution.

Publish the exact committed packages in dependency order, waiting for each
version to become visible in the crates.io index before uploading its consumer:

```sh
cargo publish --locked -p neocogi-ui-contracts
cargo publish --locked -p neocogi-ui
cargo publish --locked -p neocogi-ui-dylib
```

Keep Cargo's package verification enabled for every upload. Confirm the registry
versions, downloaded archive checksums and embedded source commit, then run an
independent consumer against registry dependencies without local patches.
Follow Cargo's [publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html).
Record the release commit/tag and confirm registry-only checksum resolution.
Local patches establish archive completeness and interoperability; registry
ownership, acceptance and index visibility are checked at upload time.
