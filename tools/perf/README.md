# Vulkan performance measurements

Use the existing `min-size` profile, `dynamic,vulkan` features, and full demo scene.
Linux measurements require `perf`, a C compiler, Vulkan development headers and
loader, and access to the native graphics session.

Generate an explicit development override for the sibling owned repositories:

```sh
python3 tools/perf/profile_vulkan.py --local-overrides /tmp/neocogi-local.toml
cargo --config /tmp/neocogi-local.toml build --profile min-size -p neocogi-demo-full --no-default-features --features dynamic,vulkan
```

The generated file selects local crates through Cargo's registry patch mechanism.
The workspace dependencies keep their published version declarations. Cargo updates
the development lockfile when using overrides; published releases require the
corresponding dependency releases and normal registry resolution.

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
