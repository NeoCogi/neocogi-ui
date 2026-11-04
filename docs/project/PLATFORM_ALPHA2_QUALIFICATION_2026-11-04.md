# Platform alpha.2 adoption, November 4, 2026

UI revision `985551d3` selects published `rs-math3d = 0.20.1`, the complete
Platform `0.1.0-alpha.2` cohort, and its required COM/host/IDL `0.1.0` SDK.
The lockfile contains registry sources and checksums, with no source patches.
The UI workspace version was `0.1.0-alpha.1` at that implementation revision;
release preparation subsequently advances the UI cohort to `0.1.0-alpha.2`.

## Rendering contract

GFX retains streamed vertex buffers and its frame uniform buffer across frames.
Vertex capacity grows only for larger geometry; exhausted slots return Busy
without replacement allocation. Shorter uploads clear unwritten tails. Suzanne
retains static vertex/index buffers and a streamed camera buffer, and binds its
explicit uniform range. Atlas candidates remain static and publish atomically
after native preparation.

The runtime retires completed native owners before widget painting can upload
camera data. GFX also maintains its host before its own uploads, supporting
standalone compositor callers. Maintenance errors retain their original owned
status/message and stop painting/submission. All capabilities remain private to
their responsible runtime, compositor or widget.

## Verification

The checks ran sequentially against registry dependencies:

- Workspace all-target checking passed with `--locked --offline`.
- Broad UI/library/integration/doc suites passed 655 tests, with 8 explicitly
  ignored native/doc cases. After the maintenance change, all 298 runtime unit
  tests and 6 compositor integration tests passed, including ordering, original
  error ownership and recovery.
- Final GFX tests passed 42 cases, including persistent owner reuse, three pinned
  slots, Busy without growth, larger-frame replacement and shorter-tail clearing.
- All 9 native GL integration tests passed. These exercise actual pixels,
  texture/uniform snapshots, immutable surface output, permissions, lifetimes and
  exact quota retirement. The optional profiling case was excluded.
- The packaged Vulkan demo passed all 22 tests, including theme changes and wheel
  routing, plus the native shader-failure integration test.
- Strict Clippy passed for the affected Vulkan crates and native GL tests.
- WGPU and WebGL full-demo targets, including tests, compiled successfully.
  These were compilation checks, not WGPU/browser execution tests.
- Three independent UI event consumers and the independently generated renderer
  control passed. The linked GL smoke application passed with its independent
  extension, generic graphics, mutable index-buffer recovery and teardown.
- All 70 Python tooling regressions passed, and derived package inputs are current.

## Ordinary release build and measurements

The executable was built from clean with the normal profile and features:

```sh
CARGO_BUILD_JOBS=2 cargo build --locked --offline --profile min-size \
  -p neocogi-demo-full --no-default-features --features dynamic,vulkan
```

The job limit affects compilation only. No optimization/profile flags or local
dependency overrides were added. The original `cargo run --profile min-size ...`
command now selects the same published dependency set directly.

The user reported **more than 1,950 FPS** in an interactive `min-size`,
`dynamic,vulkan` run after this migration, exceeding the earlier approximate
1,600 FPS target. This user observation is recorded separately from the
instrumented presentation runs below.

Three independent processes each presented 30,000 frames, excluding the first
2,000 presentations from frame timing. No builds ran during measurement. The
observer retained every completed run and reported no presentation failures.
CPU time covers the entire process, including startup and cleanup; Vulkan stage
durations are wall time inside the named calls after warmup.

| Run | Warmed FPS | Process CPU ms/frame | Acquire ms/call | Fence wait ms/call |
| --- | ---: | ---: | ---: | ---: |
| 1 | 637.336 | 0.522455 | 0.767062 | 0.314643 |
| 2 | 900.309 | 0.518886 | 0.584838 | 0.035472 |
| 3 | 459.590 | 0.521522 | 1.621846 | 0.062942 |

Median throughput was **637.336 FPS**, with a **459.590–900.309 FPS** range;
median process CPU cost was **0.521522 ms/frame**. Large acquisition/wait
durations alongside stable CPU cost demonstrate sensitivity to presentation
conditions in these instrumented runs. Desktop presentation conditions were not
controlled; the user's interactive result remains a separate observation.

The previously retained alpha.1 distribution's libraries had been overwritten
by a later debug test run. It was excluded from throughput comparison; no
controlled before/after speedup is claimed. Its separately recorded exact-profile
size data remains available for the deployment comparison below.

## Deployment size and evidence

The stripped executable and six required shared libraries total **2,912,608
bytes**, compared with the previously recorded optimized ordinary-profile total
of **2,913,512 bytes**: **904 bytes smaller (0.031%)**. Assets and themes remain
3,119,161 bytes in each retained distribution. The size comparison uses the
recorded exact-profile baseline, not the overwritten debug libraries.

The [machine-readable record](PLATFORM_ALPHA2_QUALIFICATION_2026-11-04.json)
contains dependency checksums, native artifact hashes/sizes and all three timings.
Local build/test logs, raw timing output and the independently runnable release
distribution are under `/tmp/neocogi-ui-platform-alpha2`; the current executable
is `/tmp/neocogi-ui-platform-alpha2/dist/neocogi-demo-full`. Original size evidence
is `/tmp/neocogi-ui-vulkan-profile/final-sizes.json`.

See the [benchmark guide](../../tools/perf/README.md) to reproduce current
registry builds and measurements. The [earlier investigation](VULKAN_PERFORMANCE_2026-11-04.md)
records the original alpha.1 profiling work and its historical local overrides.
