# Native Cargo builds and browser composition

Cargo owns application compilation, dependency resolution, feature selection and
native artifact paths. There is one workspace lockfile. There are no generated
component workspaces, wrapper library packages, assembly variants or `plugins.txt`.
IDL generation belongs to each consumer’s build script, reading shared subsystem sources.

## Build and run

The repository pins nightly-2026-09-29 in `rust-toolchain.toml` and enables Cargo
artifact dependencies in `.cargo/config.toml`. Rustup installs the selected tools.
Artifact dependencies currently require [nightly Cargo](https://doc.rust-lang.org/cargo/reference/unstable.html#artifact-dependencies). The COM ABI, macros and
IDL library retain their stable Rust 1.85 boundary; the generic native
staging crate use nightly Rust. External stable consumers can use the SDK from their
own workspaces.

```sh
cargo run -p neocogi-demo-full
cargo run -p neocogi-demo-full --release --no-default-features --features dynamic,vulkan
cargo run -p neocogi-demo-basic --bin neocogi-demo-calculator
cargo run -p neocogi-demo-smoke -- --smoke
```

Native applications default to `dynamic,glow`. Cargo features are additive:
alternate renderers and monoliths require `--no-default-features`. Choose one
renderer and one linkage per build. The application manifest forwards selections
to its composition; the renderer owns the exclusive backend check.

```sh
cargo run -p neocogi-demo-full --no-default-features --features dynamic,wgpu
cargo run -p neocogi-demo-full --no-default-features --features monolith,glow,embed-assets
cargo run -p neocogi-demo-smoke --no-default-features --features dynamic-extension,glow -- --smoke
cargo run -p neocogi-demo-smoke --no-default-features --features monolith-extension,glow -- --smoke
```

`NEOCOGI_DEMO_FRAMES=8` selects a finite full/basic/editor run for acceptance.
Only an absent variable disables the limit. A present value must decode to a
positive `u32`; empty, zero, malformed, overflowing and undecodable native values
fail at startup with the frame-limit diagnostic. Only successfully presented
frames count toward the limit.
The smoke command has its own deterministic `--smoke` behavior.

## Platform dependencies

Native renderers need the platform SDL2 development library and an available GPU
backend. Vulkan also links the platform Vulkan loader. CI installs SDL2, Vulkan
and software Mesa drivers on Linux; macOS uses Homebrew SDL2. Windows CI installs
`vcpkg install sdl2:x64-windows`, adds its library directory to `LIB` and its DLL
directory to `PATH`. Cargo owns NeoCogi artifacts; those OS dependencies retain
their usual platform installation requirements. Browser builds use WebGL2 and
require neither SDL nor native plugin artifacts.

## What Cargo builds

Implementation packages own reusable `rlib` targets. Their minimal `dylib/`
packages own final `cdylib` targets. Dynamic composition requests these final
artifacts. Static composition uses normal
Rust dependencies and registers their descriptor accessors.

The existing `neocogi-demo-host` handles application startup. Its optional
`neocogi-demo-plugins` dependency contains the native artifact declarations:

```toml
[dependencies]
ui-library = { package = "neocogi-ui-runtime-dylib", path = "../../crates/ui/runtime/dylib", artifact = "cdylib" }
```

That dependency has `lib = false`, Cargo's artifact default. It builds a DLL
without linking the provider's Rust library into the host. A compiler-only macro
copies the selected artifact and emits only its platform filename:

```rust,ignore
let filename = neocogi_native_layout::stage_library!("CARGO_CDYLIB_FILE_UI_LIBRARY_nc_ui");
let executable = std::env::current_exe()?;
let path = executable.parent().unwrap().join("lib").join(filename);
// SAFETY: this is the trusted artifact built from the selected Cargo graph.
// runtime was supplied by the executable entry.
unsafe { runtime.load_plugin_library(&path); }
```

There is no generated startup file. Registration lists actual selected Cargo
artifacts in explicit provider order. Nodes requires UI runtime; Skin requires
GFX. Runtime registration validates those requirements without loading additional
libraries. Metadata inspection constructs no objects.

The composition crate exists because Cargo rejects naming the same provider twice
in one package as separate artifact and normal Rust dependencies. Keeping DLL
references in their own small crate permits the host's monolith/browser path to
use normal Rust providers. This crate neither invokes Cargo nor resolves packages.

## One host-owned runtime

The final executable depends on `neocogi-com-host` and initializes its one
registry through `#[neocogi_com::com_main]`. Its authored main receives `Runtime`;
application libraries accept that handle. Each plugin's typed acquisition accepts
one supplied handle, with the generated C bootstrap forwarding the immutable
host service graph. Callbacks execute in that host; plugin code owns no registry.

Reusable providers need the facade and contracts only. They are rlib-only packages
without a `dynamic-plugin` feature. Each `*-dylib` subpackage is a minimal final
link root exporting discovery through the published SDK's `return_plugin_descriptor`
helper and the implementation's existing static descriptor. Static/browser
composition registers that same descriptor directly. Additive Cargo features cannot
introduce DLL exports into reusable libraries or their peer test dependencies. There
is no COM runtime DLL, import library, native-link bridge or runtime-specific
RPATH. Generic Cargo artifact staging still copies selected provider libraries
and assets beside the executable. Platform/system dependencies remain ordinary
requirements of those providers.

Host services and loaded provider code remain process/Wasm-instance-lived, so
returned COM objects and browser callbacks can outlive main's invocation.
See the [portable host/provider example](https://github.com/NeoCogi/neocogi-com/blob/92fa66ebc70888f71f2d07f2bbc14335a2ffd4e6/examples/com-plugin/README.md).

## Interfaces and implementation

NCIDL declares ordinary interfaces and values. A provider's creation methods are
ordinary interface methods with typed arguments. Concrete storage and plugin
publication live in its Rust implementation crate:

```rust,ignore
#[neocogi_com::com_object(api::IProvider)]
struct Provider;

#[neocogi_com::plugin(name = "example.provider")]
fn acquire_root(_runtime: neocogi_com::Runtime) -> Result<neocogi_com::ComPtr<api::IProviderAbi>, neocogi_com::NcStatus> {
    Provider::from_value(Provider).query()
}
```

The return interface supplies root identity and threading. Version defaults to
Cargo package version; `dependencies = [PLUGIN_ID]` declares hard providers.
The macro generates `plugin_descriptor()` for static registration. These reusable
packages leave its optional dynamic export gate undeclared; the final `*-dylib`
package alone exports `nc_get_plugin_descriptor_v1` and delegates to that descriptor. No `class`/`factory`
IDL roles or factory bootstrap traits are required. A finished service is also a
valid plugin root. See [IDL modules](https://github.com/NeoCogi/neocogi-com/blob/92fa66ebc70888f71f2d07f2bbc14335a2ffd4e6/docs/contracts/IDL_MODULES.md) and the
[macro reference](https://github.com/NeoCogi/neocogi-com/blob/92fa66ebc70888f71f2d07f2bbc14335a2ffd4e6/crates/macros/README.md).

Consumers need generated interface definitions, not a Rust dependency on their
implementors. NCIDL imports read declarations at generation time. A public
construction method still belongs in an accessible contract; private storage and
bootstrap policy do not. Bindings can live in an API crate or an application's
`OUT_DIR`. Reuse the same Rust binding definitions when ordinary Rust helper APIs
exchange those types; independently generated definitions are nominally distinct.
Across DLLs, negotiation uses the matching C ABI contract instead.

## Resources and installed applications

Native file-backed resources are copied during compilation into `assets/` and
`themes/` beside the executable. The full-demo photo is copied to
`assets/FACEPALM.png`. Runtime resource paths start from `current_exe()`;
the working directory and source checkout are irrelevant. Relative smoke-test
`--theme` paths use that same base; explicit absolute user paths remain supported.
`embed-assets` compiles the existing resource bundle instead and omits resource
staging. Theme changes retain the original application asset base.

The resulting layout is:

```text
target/<profile>/
  neocogi-demo-full[.exe]
  lib/                       # selected provider libraries
  assets/                    # file-backed resources, unless embedded
  themes/                    # file-backed themes, unless embedded
```

This profile directory is an additive development cache. Create a fresh exact
distribution with the repository's packager:

```sh
python3 scripts/package-native.py --package neocogi-demo-full --bin neocogi-demo-full --profile min-size --no-default-features --features dynamic,vulkan --output /tmp/neocogi-app
```

The destination must not exist. Cargo exports the selected executable into private
scratch while holding its build lock; the production macros stage selected DLLs
and recipe resources there. The packager checks exact file membership, private
paths and native loader metadata, then publishes the complete directory by rename.
It requires `readelf` for ELF, `llvm-readobj` for PE or `otool` for Mach-O inspection.
Move that directory as a unit to relocate the application. Compiler caches and debug
unremapping sidecars stay on the build machine. Workspace profiles use
Cargo's `trim-paths = "all"` so panic/debug/compiler source locations do not
disclose checkout or Cargo-home paths. **`*.trim-paths.json` sidecars deliberately
contain original paths for local debugging and must stay on the build machine.**
Compiler path trimming does not sanitize arbitrary application string literals;
artifact paths therefore never appear in generated runtime code.

Staging reads exactly Cargo-selected artifact paths and literal resource inputs;
it invokes no Cargo subprocess and constructs no dependency graph. It tracks
source files, resource directory membership and copied files in compiler dep-info.
Deleted outputs and cached renderer switches trigger copying again. Identical
contents retain their timestamps; changed libraries are replaced atomically,
without truncating a mapped Unix library. An in-use Windows DLL can reject
replacement: stop the application before rebuilding. Removed optional providers
or resources may leave unused files in a shared profile directory; registration
loads only the compiled selection. The packager instead starts with empty private
storage and admits only the file set reported by that Cargo invocation. Basic demos
include recipe fonts/icons; full demos additionally include photo/mesh and the
selected authored themes' JSON-relative resource closure. Smoke includes external
themes without full-only photo/mesh. Embedded compositions stage no resource files.
All adjacent recipe inputs require regular source components inside the checkout;
theme resource references must also be relative. Absolute references and symlinks
fail compilation. Copied theme
JSON uses normalized relative references, so discarded `directory/..` prefixes do
not require extra directories in the distribution. General theme-authoring APIs
may still accept absolute user paths outside this adjacent-resource recipe.
Restoring outputs during macro expansion can require one following compilation
to settle Cargo's timestamp-based dep-info; subsequent unchanged builds are fresh.

The pinned Cargo version marks each profile output directory with `.cargo-lock`.
Staging locates that marker among ancestors of the consuming package's `OUT_DIR`;
it never reconstructs hashes or infers a second graph. With a separate Cargo
build directory, set the absolute build-only `NEOCOGI_NATIVE_OUTPUT_DIR` to the
actual executable directory. Unsupported layouts fail compilation explicitly.
This private integration is tied to the pinned Cargo layout. Distribution
installation, signing and platform SDL/Vulkan deployment remain separate tasks.

Concurrent packagers may share Cargo's supported, locally locked cache, including
separately configured target/intermediate directories. No wrapper build lock is
used. Each invocation owns its exported executable and staged files. A destination
name lock gives cooperating packagers one winner; existing destinations are never
overwritten. The sibling `.NAME.lock` file deliberately persists so waiters share
one lock identity. Arbitrary external edits and filesystems where Cargo or OS
locking is unavailable (including unsupported NFS layouts) are outside this
concurrency guarantee. Unsupported publication locking fails closed.

## Profiles and output

Use Cargo's ordinary `--release`, `--profile`, `--target` and `--target-dir` options.
Both release and min-size retain fat LTO, one codegen unit and symbol stripping.
The min-size profile also selects immediate abort, removes Rust panic locations
and debug formatting, and disables forced unwind tables for deployed target code.
Build scripts and procedural macros retain normal host panic/diagnostic settings.
All demo and benchmark entry points convert startup results through the shared
`neocogi_demo_host::report_exit` helper. It formats any `Display` error (including
boxed errors and COM status codes) and returns an explicit success/failure exit
code. Native failures go to stderr and browser failures to the JavaScript console;
ordinary `Result` termination would use stripped `Debug` and lose these diagnostics.

Cargo's abort-test harness preserves the target panic strategy while isolating
panic expectations in child processes, including min-size profile tests.

```sh
cargo run -p neocogi-demo-full --no-default-features --features dynamic,vulkan --profile min-size
# The executable uses the adjacent lib/ and resource directories:
./target/min-size/neocogi-demo-full
```

The workspace enables Cargo's `build-std` for std and its test/procedural-macro
support, preserves normal backtrace/unwind features, adds `optimize_for_size`, and installs
`rust-src` through the pinned toolchain. This is necessary because Cargo has no
profile-specific `build-std` switch: a named profile alone cannot rebuild std.
Target std is consequently rebuilt for other workspace profiles too, with an
initial compile/cache cost for each target/profile. Portable SDK consumers outside
this nightly development workspace keep their normal prebuilt standard library.
The `cargo build-min-size` alias is only shorthand for `cargo build --profile min-size`;
there is no separate target, wrapper, output layout or hidden compiler policy.

The min-size profile retains full `Debug` formatting inside `naga` and `naga-types`.
Their GLSL generator uses it for executable float literals and stage-specific
symbol names. Stripping it breaks shader syntax or merges independent bindings.
Other target packages retain the profile's normal diagnostic-stripping policy.

A dynamic artifact request builds a final cdylib-only package with its ordinary
implementation dependencies. Static/monolith/browser consumers link rlib-only
implementation packages and do not emit unused DLLs. Keep deployment artifacts as
normal dependencies: build dependencies receive host-tool profiles even with
`target = "target"`, losing application optimization/panic/size flags. The final
packages add only symbol ownership, with no duplicate metadata, factories, runtime,
generated workspace or dependency resolver.

The host implementation Rust target is `neocogi_com_host`. Native provider names remain `nc_ui`, `nc_ui_nodes`, `nc_shader_compiler`, `nc_render`,
`nc_gfx`, `nc_skin` and optional `nc_ui_extension`. The executable is in the usual
`target/<profile>` directory when no explicit target is selected. Dependency DLLs
remain in Cargo's artifact directories as compiler inputs, with deployment copies
in `lib/`. Registration joins those filenames to the executable directory.
Do not reconstruct hashes or cache paths across builds.
An explicit `--target` adds Cargo's normal target-specific output directory.

## Debugging in VS Code

Open this repository root as the workspace folder and install its recommended
`rust-lang.rust-analyzer` and `vadimcn.vscode-lldb` extensions. In Run and Debug,
select **Full demo: Vulkan (shared, debug)** or **Full demo: Glow (shared, debug)**
and press F5. CodeLLDB invokes the ordinary Cargo build and selects its reported
binary; no target/cache path is reconstructed. These configurations use `dev`,
which retains unoptimized code and debug information. Release/min-size remain
stripped and are unsuitable for source stepping.

The workspace settings also select CodeLLDB for rust-analyzer's Debug action.
Before launching an application, CodeLLDB imports
[`scripts/lldb_source_map.py`](../../scripts/lldb_source_map.py). It reads the
selected executable's adjacent `.trim-paths.json`, the actual version-1 mapping
format emitted by the pinned Cargo. The file supplies the inverse mappings for
workspace files, `OUT_DIR` bindings, registry dependencies and rebuilt std; the
debugger appends those mappings after any explicit source-map rules. This avoids
hard-coded Cargo prefixes, registry hashes, toolchain names and target directories.
An untrimmed executable without a sidecar needs no mappings. Unsupported or
malformed metadata produces a debugger import error.

Set breakpoints directly in provider Rust source before starting. They can remain
unresolved until the provider DLL loads; CodeLLDB then binds them and supports
ordinary stepping and local-variable inspection. For example, a breakpoint on
the call to `create_runtime_from_assets` in `crates/ui/runtime/src/lib.rs` stops
inside the copied `libnc_ui.so`. Generated COM client/thunk frames should open
their Rust file under `OUT_DIR`, rather than disassembly.

The mapping file stays on the build machine and is not copied into `lib/`.
Debug symbols in the copied ELF libraries are preserved. The native acceptance
test scripts still use their copied test layout; launching an acceptance test's
compiler-directory binary directly does not supply adjacent resources. Separate
PDB/dSYM staging and Windows/macOS debugger execution are not covered by the
Linux checks alone. See the [current debugger validation scope](../../crates/ui/runtime/docs/TESTING.md).

## Browser

Browser builds use normal Rust dependencies, one static runtime, linked plugin
descriptors and embedded resources. They select neither native composition nor
native runtime artifact dependencies:

```sh
cargo check -p neocogi-demo-full --target wasm32-unknown-unknown --no-default-features --features webgl
scripts/web-demo.sh build
```

The existing web script performs browser binding generation and site preparation;
it does not build a second native Cargo graph.

## Validation

`scripts/test-demo.sh` and `scripts/test-extension.sh` execute normal Cargo
builds and application tests.
The build itself copies its selected files. `python3 scripts/test_package_native.py
--production /tmp/neocogi-packaging-tests` checks fresh composition membership and
barrier-controlled concurrent capture/publication with actual applications and
production macros. Use a new test output directory for each run.
`scripts/test-native-layout.py --cache
--run` checks actual Linux binaries, cached backend switches, deleted outputs and
a renamed full application directory. The plugin regression also relocates its
test host and calls a separately built DLL without loader environment overrides.
Cargo nests unit-test executables in compiler directories. The acceptance scripts
use `scripts/run-native-test.py` as a Cargo runner to copy an already compiled
test and its staged inputs into a temporary application directory. This test-only
runner never builds; production startup always uses adjacent files directly.
SDK cross-compiler and ABI qualification belong to the upstream COM workspace.
The [current native validation guide](../../crates/ui/runtime/docs/TESTING.md)
lists maintained entrypoints and platform limitations.

The required CPU shader provider is staged and registered before the renderer.
Native dynamic hosts reference its cdylib as a Cargo artifact; native monoliths
and WebGL assemblies register its statically linked descriptor before activating
the selected renderer. The renderer uses the SDK runtime client API without
linking a host implementation or a fallback compiler.
