# Vulkan performance investigation, November 3–4, 2026

The week-old build reproduces the reported 1,400 FPS performance. The regression
already exists before the crates.io Platform migration. Checked geometry,
per-vertex serialization, and native Vulkan allocation churn account for the
measured increase in CPU work. Local fixes preserve the existing `min-size`
optimization settings, checked geometry, immutable captures, and completion ownership.

The fixes span `neocogi-ui`, `../rs-math3d`, and `../neocogi-platform`, on local
`perf/vulkan-frame-throughput` branches. Dependency releases remain unpublished;
use the explicit local Cargo overrides below to exercise all three together.

## Configuration and comparison

The application selection is the user's original command:

```sh
cargo run --profile min-size -p neocogi-demo-full --no-default-features --features dynamic,vulkan
```

Hardware: Intel Core Ultra 7 155H and Intel Arc Graphics (MTL), with Mesa
26.0.8-1ubuntu0.3. The desktop session is Wayland. `perf` samples confirm that the
application maps `libvulkan_intel.so`. Rust is `nightly-2026-09-29`; perf is 7.0.14.

The profile retains `opt-level = "z"`, fat LTO, one codegen unit, immediate abort,
and the existing formatting/path settings. Symbolized investigations override only
`profile.min-size.debug=2` and `profile.min-size.strip="none"`. Final qualification
uses the ordinary stripped profile. No frame, scene, shader, or precision work is
removed to improve the measured result.

Initial whole-process measurements use 10,000 frames. CPU time is `task-clock`
divided by frame count; elapsed FPS includes startup, native waits, and teardown.

| UI revision | Meaning | CPU ms/frame | Elapsed FPS |
| --- | --- | ---: | ---: |
| `55b305cc` (October 28) | Week-old comparison | 0.556 | 1,391 |
| `a2fb4af7` (November 1) | Immediately before checked UI geometry | 1.046 | 851 |
| `85a86a48` (November 2) | Immediately before published Platform migration | 1.490 | 437 |
| `323a5322` (November 3) | Current reported regression | 1.478 | 646 |

The first historical interval includes the October 30 generic-renderer changes;
the second includes checked UI geometry and subsequent changes. These are measured
intervals rather than a claim that a single commit accounts for each entire delta.
The similar pre/post-migration CPU times rule out the registry transport change
as the source of the large regression.

Three alternating 3,000-frame runs initially gave median CPU times of 0.573 ms
for October 28 and 1.478 ms for current code: 2.58 times as much CPU work. Elapsed
FPS varies more with desktop scheduling and presentation. Later runs therefore
also measure warmed intervals between actual Vulkan presentation entries.

## Profile evidence

The unchanged stripped executable's user-space self samples divide approximately
46.0% into UI, 32.8% into GFX, 9.9% into libc, 3.7% into UI nodes, 3.2% into the
Intel Vulkan driver, and 2.1% into Platform. These samples exclude kernel time,
blocked time, and GPU execution; the driver percentage alone cannot bound native
rendering cost.

Named self samples from the symbolized build include:

| Function | User-space samples |
| --- | ---: |
| `Fixed<i64,12>::checked_sum_products_pair` | 7.61% |
| `Vec<u8>::spec_extend` | 7.49% |
| `Graphics::geometry` append closure | 6.32% |
| UI geometry subtraction wrapper | 5.79% |
| Exact wide `Accumulator::add` | 5.74% |
| libc `memmove` | 5.29% |
| `atlas_rect_triangles` | 4.22% |

DWARF unwinding is incomplete with the profile's disabled unwind tables. These are
flat self samples, with no lost samples; inclusive caller percentages are not used.
Disassembly independently confirms three checked product-pair calls in the old
2D `cross2`: the general 3D cross product computes two unused zero components.

For 1,000 frames, syscall counts show the second major cost:

| Build | `ioctl` | `mmap` | `munmap` |
| --- | ---: | ---: | ---: |
| October 28 | 28,206 | 410 | 85 |
| Current | 45,223 | 9,409 | 14,070 |
| Buffer and descriptor reuse candidate | 28,905 | 440 | 124 |

A separate 100-frame DRM trace counted 35 versus 432 buffer-create operations and
29 versus 921 mapping-offset operations for old versus current code. Syscall
counts establish allocation churn; ptrace timings are not used as frame benchmarks.

## Implemented changes and preserved contracts

### Math

`rs-math3d/src/fixed/mod.rs` uses an exact signed `i128` kernel for two products,
then rounds once and checks the final storage range. The only possible positive
pair overflow cannot fit any supported storage even after the maximum fractional
shift. Longer sums retain the wide accumulator needed for later cancellation.

Checked addition and subtraction use the sealed storage integer's overflow
operations, avoiding widened arithmetic and narrowing for same-scale operations.
Small checked vector adapters inline into their callers. Direct f32 decoding uses
the actual 24-bit significand while preserving nonfinite rejection, signed range
checks, subnormals, and ties-to-even quantization.

Regressions compare 1,012,500 signed product-pair cases against the exact wide
accumulator, exhaust all i8 operand pairs at two scales for addition/subtraction,
and compare f32 conversion against the independent f64 decoder across exponent
boundaries and 100,000 deterministic raw patterns in seven formats.

### UI and GFX

[UI geometry](../../crates/ui/support/src/render/geometry.rs) computes the 2D
determinant directly through the checked scalar contract. Ear clipping reuses
candidate edges and rejects probes outside closed bounds before exact orientation
checks. The admitted coordinate domain, 1/64-pixel rounding, large-origin areas,
closed boundaries, and atomic failure behavior remain intact.

[GPU packing](../../crates/gfx/impl/src/compositor/gpu.rs) encodes explicit
little-endian fields into pre-sized 20-byte rows. Whole slices are written per
batch; row allocations flatten into bytes without copying or casting Rust/IDL
layouts. Tests cover shader bytes, triangle-aligned chunk boundaries, quad winding,
batch order, empty input, and invalid ranges.

[Composition](../../crates/gfx/impl/src/compositor.rs) normalizes glyph UV endpoints
once per immutable sorted atlas revision. Metadata, ASCII/Unicode fallback slots,
and cached coordinates publish together. Nonbinary atlas dimensions and revision
changes are checked bit-for-bit against the original divisions. Nine-patch cell
traversal preserves immediate image/diagnostic order with one iterator.

### Platform Vulkan

`../neocogi-platform/crates/render/gpu/src/backends/vulkan/generic.rs` keeps
host-visible allocations mapped and reuses exclusive retired buffer capacity.
The pool retains at most 64 allocations and 4 MiB of actual native memory. Before
reuse, the whole capacity is cleared and initialized; noncoherent memory is flushed.

Descriptor pools return through the same final-owner channel, with bounds of 32
pools and 8,192 total sets and descriptors in each category. Checkout resets a
compatible pool before publishing new descriptor handles. Neither pool retains
resource identity, client authority, a source version, or a producer endpoint.

Prepared owners and submission fences still prevent early reuse. Ended logical
version charges refund at retirement; retained physical capacity remains counted
until destruction. Allocation misses release idle capacity before requesting new
storage. Failure and shutdown destroy idle capacity, and disconnected notifications
recover owned packets for late destruction. Native tests exercise retained clones,
pixel output after reset/reuse, shorter replacement tails, accounting, native
allocation limits, device loss, and aggregate pool bounds.

## Final measurements and validation

The final stripped application was timed in three alternating comparison rounds
and one additional control pair using an independently rebuilt, unchanged baseline
with the identical ordinary `min-size` profile. All four optimized measurements
used the same final executable and deployment, 15,000 frames per run, and 1,000
warmup presentations. Every run completed all presentations with zero errors.

Across those four optimized runs, warmed FPS was **1,595 median**, with a range of
916–1,730. Whole-process CPU time was **0.535 ms/frame median**, ranging only
0.532–0.540 ms/frame. The additional matched-profile control measured **727 FPS
before and 1,709 FPS after**, with CPU time falling from 1.390 to 0.532 ms/frame.
The three-round cohort alone measured 682 FPS median for the regressed build and
1,482 for the optimized build; all individual results are retained in the
[measurement summary](VULKAN_PERFORMANCE_2026-11-04.json).

The remaining FPS spread accumulates inside Vulkan fence waits. In the three-round
cohort, optimized fence-wait totals varied from 0.89 to 8.25 seconds over roughly
14,000 timed intervals, while CPU work remained nearly constant. The week-old build
also showed this effect: 877–1,455 FPS with 2.97–9.72 seconds in fence waits.
These measurements locate the residual wait without attributing it to a particular
GPU execution, presentation, or scheduling cause. The CPU/allocation regression
and native wait variance are separately visible in the retained data.

Matching ordinary-profile binaries, including the executable and six shared
libraries, total **2,913,144 bytes before** and **2,913,512 bytes after**: an increase
of **368 bytes (0.013%)**. Deployed assets and themes match byte-for-byte. Earlier
stripped symbol-build estimates are superseded by this matching-profile comparison.
No optimization level or other `min-size` setting was changed. The Vulkan allocator
trades repeated native creation for the explicitly bounded idle capacity documented
above.

Fresh selected suites passed **907 tests and doctests**:

| Suite | Passed |
| --- | ---: |
| Math unit, integration and documentation tests | 194 |
| UI runtime, nodes and support | 509 |
| UI public API, document and consumer fixtures | 71 |
| GFX library and allocation-reservation integration | 46 |
| Native Vulkan renderer, with local math overrides | 86 |
| UI/Vulkan integration | 1 |

The math `no_std`/libm check also passed. Strict Clippy checks passed for UI
providers, GFX and Platform. The math repository's strict all-target Clippy check
has existing failures: a pristine `15fa8459` archive and the optimized tree emit
92 identical diagnostics when compared by message and primary source text. No new
math lint finding was introduced. Existing ignored tests retain their prior status.
The benchmark observer compiled with `-Wall -Wextra -Werror` and was exercised on
all retained comparison executables.


## Implementation commits

All repositories use local branch `perf/vulkan-frame-throughput`:

| Repository | Commit | Change |
| --- | --- | --- |
| rs-math3d | `16d9be02` | Exact checked arithmetic specialization |
| Platform | `60387d9e` | Completion-owned buffer and descriptor reuse |
| UI | `11e57302` | Checked 2D predicates and cached ear edges |
| UI | `88eaecaa` | Bulk packing, exact glyph UVs and cell traversal |

## Reproduction and retained evidence

Generate local registry overrides and build using the existing profile:

```sh
python3 tools/perf/profile_vulkan.py --local-overrides /tmp/neocogi-local.toml
cargo --config /tmp/neocogi-local.toml build --profile min-size -p neocogi-demo-full --no-default-features --features dynamic,vulkan
python3 tools/perf/profile_vulkan.py --exe optimized=target/min-size/neocogi-demo-full --output /tmp/neocogi-vulkan-results
```

The [benchmark tool](../../tools/perf/README.md) records every warmed FPS result,
whole-process CPU time, and Vulkan synchronization duration. Multiple `--exe`
arguments alternate retained distributions. Its optional preload observer is
outside the product binaries. Build and benchmark jobs run sequentially.

Raw profiling data, reports, historical build logs, repeated timings, and retained
comparison distributions are local to `/tmp/neocogi-ui-vulkan-profile`. Important
files include `current-stat.txt`, `oct28-stat.txt`, `pre-migration-stat.txt`,
`current-symbol-flat.txt`, `cross2-assembly.txt`, the `*-syscalls.txt` files,
`qualification-progress.log`, and the final comparison JSON/logs.

Registry declarations and the `0.1.0-alpha.1` tag retain their existing meanings.
The full improvement needs the fixed math and Platform crates; ordinary registry
resolution continues to select their currently published versions until those
dependency releases are published and adopted.
