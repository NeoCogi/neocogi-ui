# Tracing generated Rust back to IDL

Every Cargo IDL build writes a readable JSON source map beside its Rust bindings:

```text
neocogi_ui_nodes.rs
neocogi_ui_nodes.rs.map
neocogi-idl-graph.json
```

Find the actual paths without searching build directories:

```sh
cargo run -p cargo-neocogi --offline -- module generate neocogi.ui.nodes
```

The JSON command result includes `bindings`, `source_map`, and `dependency_graph`.
A direct Cargo build of the contract crate produces the same files through its
existing `build.rs`. `BuildOutput::source_map` and Cargo's `idl_source_map`
metadata expose the path to build tooling.

Generated declarations and authored members also carry readable documentation:

```rust
/// IDL source: `neocogi/ui/nodes/widgets.ncidl:4:11`.
pub trait Textbox /* generated requirements and methods */ {}
```

The annotation names the originating IDL identifier. Source paths are relative
to the contract's IDL root. The sidecar supplies `source_root`, relative to the
map's directory, so tools resolve an authored file as:

```text
map directory / source_root / mapping.original.path
```

Cargo canonicalizes the two directories before computing this relative path.
If they have different filesystem roots, such as different Windows drives,
`source_root` is absolute. Pure in-memory generation leaves the root unset;
the caller supplies its filesystem context.

## What is mapped

The compiler parses the final pretty-printed Rust to measure ranges. Formatting,
documentation, long signatures and generated helper bodies are therefore included
in the coordinates. A complete declaration range covers its generated fragment;
nested syntax entries refine that origin for structs, enums, unions, traits,
methods, functions, fields, variants and helpers.

| Generated Rust | IDL origin |
| --- | --- |
| Copied struct or enum | Its declaration identifier |
| Named field or enum alternative | Its authored field or variant identifier |
| Integer enum or bit flag macro body | Its declaration and each variant/flag |
| Interface method and associated generated output methods | The method or output identifier |
| `TextboxObserver` | The `Textbox` declaration that requests an observer |
| Observer `changed` method and `TextboxEvent::Changed` | The producer's `changed` output |
| Message wire record and codec helpers | The originating message declaration |
| Message case helper and borrowed named fields | The originating variant and field |
| Imported re-export | The local IDL `use` statement |

A generated helper without a distinct authored counterpart inherits its nearest
known declaration or member. Positional fields inherit their variant origin.
Origins identify the authored cause; they are not a line-for-line translation
of generated algorithm bodies. Module constants such as `MODULE_ID` have no
individual authored declaration and remain unmapped.

The map describes the emitted `.rs` file **before Rust macro expansion**. It does
not describe a separately expanded Rust dump. Provenance docs also survive where
COM macros forward authored documentation to generated client methods, as in the
checked-in persistence bindings.

## JSON format and lookup

This is NeoCogi's readable range-map schema **1**, not JavaScript Source Map v3.
Each mapping has a `kind`, generated Rust `name`, `generated` range and `original`
IDL path/range. The ranges use one-based lines and one-based **Unicode scalar**
columns, matching the parser. A tab counts as one character. Starts are included;
ends are excluded. Columns are neither UTF-8 byte offsets nor UTF-16 code units.

An illustrative entry:

```json
{
  "kind": "field",
  "name": "Size::width",
  "generated": {
    "start": { "line": 28, "column": 5 },
    "end": { "line": 30, "column": 19 }
  },
  "original": {
    "path": "example/widgets/types.ncidl",
    "range": {
      "start": { "line": 3, "column": 15 },
      "end": { "line": 3, "column": 20 }
    }
  }
}
```

Tools should use the most specific enclosing range. The compiler exposes that
operation directly:

```rust,no_run
use neocogi_idl::{RustSourceMap, SourceMapPosition};
# fn example(json: &str) -> Result<(), serde_json::Error> {
let map: RustSourceMap = serde_json::from_str(json)?;
assert_eq!(map.schema, 1);
if let Some(entry) = map.lookup(SourceMapPosition { line: 28, column: 10 }) {
    println!("{}:{}:{}", entry.original.path,
        entry.original.range.start.line, entry.original.range.start.column);
}
# Ok(())
# }
```

`generate_rust_with_source_map` returns `GeneratedRust { source, source_map }`.
`generate::generate_rust_with_paths_and_source_map` additionally accepts explicit
Rust import locations. Existing string-returning generation functions remain
available and include the provenance annotations; callers needing a map should
use the paired result and persist both artifacts together.

Maps and annotations do not enter declaration IDs, ABI fingerprints, runtime
registration, or plugin loading. Moving IDL declarations or renaming source parts
updates provenance while preserving the same contract ABI. Cargo writes unchanged
outputs only when their contents differ. These are build/debugging artifacts and
are not automatically copied into application deployment packages.

This provides provenance for humans and tools. Rustc diagnostics and editor
Go to Definition are not automatically redirected by the sidecar; either feature
would require a consumer that reads these mappings.

## Validation

Tests cover Unicode columns, multipart ordering, formatting, macro enum/flag
bodies, message fields/cases, derived observers, callback bodies, imports, EOF
boundaries and JSON round-tripping. A real Cargo module-generation test verifies
reported map paths, resolution to the authored file, unchanged-output reuse, and
location-only edits refreshing the map without changing binding behavior.
