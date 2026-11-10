# Vulkan performance measurements

Use the existing `min-size` profile, `dynamic,vulkan` features, and full demo scene.
Linux measurements require `perf`, a C compiler, Vulkan development headers and
loader, and access to the native graphics session.

Build the published dependency set directly; no local override is needed for
Platform 0.2 alpha.2 or rs-math3d 0.21.0:

```sh
cargo build --locked --profile min-size -p neocogi-demo-full --no-default-features --features dynamic,vulkan
```

For unpublished companion development, `--local-overrides /tmp/neocogi-local.toml`
generates an optional configuration for the sibling Platform and math checkouts.
Pass it to Cargo with `--config /tmp/neocogi-local.toml`. This deliberately updates
the development lockfile; release builds use the committed registry resolution.

Measure complete intervals between presentation entries after warmup:

```sh
python3 tools/perf/profile_vulkan.py \
  --exe optimized=target/min-size/neocogi-demo-full \
  --output /tmp/neocogi-vulkan-measurement
```

Repeat `--exe LABEL=PATH` to alternate independently retained distributions. Each
executable needs its matching native `lib`, `assets`, and `themes` deployment beside
it. Build or copy each distribution before measurement; keep compilation sequential
and stop builds while benchmarks run.

The observer preserves native arguments and results. Warmed FPS includes all CPU
application work and native waits between presentation calls. `task-clock` measures
the complete process, including startup and teardown. Vulkan stage wall times
identify fence, acquisition, submission and presentation delays; those stage totals
also include final cleanup after the last presentation. This tool expects the demo's
single presentation thread and queue. Desktop activity and power management can
affect elapsed time, so retain the full range alongside the median.

Raw logs, all accepted runs and a median/range summary remain in the output directory.
The source and raw-data locations for the November investigation are recorded in the
[performance report](../../docs/project/VULKAN_PERFORMANCE_2026-11-04.md).
The [Platform alpha.2 qualification](../../docs/project/PLATFORM_ALPHA2_QUALIFICATION_2026-11-04.md)
records the current registry build, deployment size and all repeated timings.
