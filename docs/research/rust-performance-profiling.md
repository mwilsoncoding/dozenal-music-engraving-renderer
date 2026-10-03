# Rust Performance Profiling

Research for [Research Rust performance profiling](https://github.com/mwilsoncoding/dozenal-music-engraving-renderer/issues/53). The ticket is the context pointer; this report does not resolve it.

## Recommendation

**Defer adopting a profiler dependency, benchmark crate, or CI performance gate.** The README says `Dependencies: None`, and the [MVP decision](https://github.com/mwilsoncoding/dozenal-music-engraving-renderer/issues/21) requires no third-party Rust crates or external runtime tools. Optional, developer-installed profilers do not change the shipped CLI, but are not justified as a maintained project workflow until a representative workload exposes a performance question.

When a real score is slow, first record a repeatable workload and profile an optimized binary with symbols. On Linux, use `perf`; on macOS, use Instruments' Time Profiler. Flame graphs are a way to inspect sampled stacks, not a separate source of timing truth. Keep the workload, Rust/tool versions, OS, CPU, build flags, and profiler settings with any conclusion. Do not treat a single run or a cross-machine comparison as a regression.

The [Rust Project Primer performance overview](https://rustprojectprimer.com/measure/performance.html) is a useful starting map; the tool and language behavior below is grounded in first-party documentation.

## Where to Look

The current CLI reads and decodes a file, parses XML, maps MusicXML notes to tones, then renders a scene to SVG. These are useful profiling targets, not evidence that any is currently a bottleneck:

| Work | Current code boundary | Profiling caveat |
| --- | --- | --- |
| XML read/decode and parse | [`xml::read_file` and `xml::parse`](../../src/xml.rs#L307); [`parse_score`](../../src/main.rs#L121) | Whole-CLI timing includes file I/O and process startup; CPU samples show only time spent on-CPU. |
| Pitch mapping | [`parse_note` and `parse_pitch`](../../src/main.rs#L417) | Pitch conversion is nested in score parsing and may be too brief to stand out in samples. |
| Layout | [`event_geometry` and `next_event_x`](../../src/main.rs#L1348) | Called during rendering; use a score with enough events and overlap cases to exercise spacing. |
| SVG output | [`Scene::to_svg` and `Scene::write_path`](../../src/main.rs#L1593) | SVG serialization follows scene construction; compare output size and command count when interpreting its cost. |

The existing canonical fixture is useful to check that a profiling command works, but its small workload is not enough to infer performance for larger scores. If samples are sparse or dominated by startup/I/O, profiling cannot reliably rank pitch mapping or individual rendering phases. Distinguishing very short phases then needs deliberate in-process timing around existing stage boundaries; it should not be added until there is a question the measurement can answer.

## Approaches

**Sampling profilers** periodically capture active call stacks. They are suited to answering “where is CPU time going?” with less source modification than manual timers, but short functions can be missed and optimized/inlined code can blur attribution. On Linux, [`perf record`](https://man7.org/linux/man-pages/man1/perf-record.1.html) and [`perf report`](https://man7.org/linux/man-pages/man1/perf-report.1.html) are the native workflow. Installation and access to hardware/software events depend on the kernel, system configuration, and permissions; see the [kernel perf security guide](https://www.kernel.org/doc/html/latest/admin-guide/perf-security.html). `perf` availability and permissions are not guaranteed on hosted CI.

On macOS, Apple's [Instruments CPU profiling workflow](https://developer.apple.com/documentation/xcode/analyzing-cpu-profile-data) provides the native Time Profiler without a Rust crate, but requires Xcode/Instruments and macOS. [`samply`](https://github.com/mstange/samply) is an optional cross-platform sampled profiler for Linux and macOS with a Firefox Profiler UI. It can smooth over different platform interfaces, but is another external tool to install, version, and maintain. Neither it nor Instruments is a Cargo dependency.

**Flame graphs** summarize stack samples so wide stacks identify functions with more sampled CPU time. The [`flamegraph` project](https://github.com/flamegraph-rs/flamegraph) can render profiles (commonly Linux `perf` data); it does not replace collection, workload selection, or interpretation. It and samply are Apache-2.0 licensed according to their [upstream license metadata](https://api.github.com/repos/flamegraph-rs/flamegraph/license) and [samply license metadata](https://api.github.com/repos/mstange/samply/license). This is permissive for optional tooling, but does not remove its separate release/update burden. Do not vendor or install either through this app's Cargo manifest for the MVP.

**Instrumentation** adds explicit timers, for example `std::time::Instant`, around selected operations. `Instant` is in the standard library and measures elapsed time ([Rust documentation](https://doc.rust-lang.org/std/time/struct.Instant.html)); it keeps the no-crate rule and can distinguish phases that sampling cannot. However, timers must be added at meaningful boundaries, can perturb small operations, and report only what was instrumented. A wall-clock CLI timer also includes startup and I/O, not just parser CPU.

**Benchmarks** answer “did this defined operation get slower?” They need fixed inputs, controlled builds, repeated measurements, and comparison methodology; they do not by themselves explain the cause. Cargo's [`cargo bench`](https://doc.rust-lang.org/cargo/commands/cargo-bench.html) runs benchmark targets, while [Criterion](https://bheisler.github.io/criterion.rs/book/) supplies statistical measurement and comparison. Criterion would add a dev-dependency and dependency tree, conflicting with the current no-crates rule; its upstream license is Apache-2.0 ([metadata](https://api.github.com/repos/bheisler/criterion.rs/license)). External repeated CLI timing avoids a crate but is noisy and measures the whole process. Neither route is warranted as a CI gate without a target workload and accepted regression threshold.

## Practical Workflow

Cargo's release profile omits debug info by default. Its documented `debug` setting can retain profiling symbols without changing `Cargo.toml`; `1` requests limited debug info, while `2` includes full info ([Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)). For example:

```sh
CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release
perf record --call-graph dwarf -- target/release/domunor tests/fixtures/canonical.musicxml --output /tmp/canonical.svg
perf report
```

The canonical input above is only a smoke test. For useful samples, use a fixed, representative larger score and repeat the same workload; avoid adding process-launch loops that mostly profile startup. On macOS, launch the same release binary and input from Instruments' Time Profiler. Keep output files outside the repository. If exact short-phase durations are needed, use a separate instrumented build and compare its timings cautiously with the uninstrumented profile.

Profiler output is diagnostic, not a stable CI metric. CPU contention, power management, thermal state, event availability, compiler version, and OS/hardware differences can move results. For reproducibility, preserve the input and command, report build/tool versions and machine details, run several times under low background load, and compare like-for-like distributions. Linux CI may deny perf events; macOS runners are not a fixed local machine. Keep ordinary correctness tests in CI and defer performance thresholds until repeated measurements establish a meaningful baseline.

If a bottleneck is found, first use a profiler to explain its call-stack cost. Only then define a narrowly scoped benchmark for that operation or a fixed end-to-end fixture to detect future regressions. Reassess Criterion and its transitive dependencies against the MVP policy at that point; do not conflate a benchmark result with hotspot diagnosis.

## Primary Sources

- [Cargo profile settings](https://doc.rust-lang.org/cargo/reference/profiles.html), [`cargo bench`](https://doc.rust-lang.org/cargo/commands/cargo-bench.html), and [`std::time::Instant`](https://doc.rust-lang.org/std/time/struct.Instant.html)
- Linux [`perf record`](https://man7.org/linux/man-pages/man1/perf-record.1.html), [`perf report`](https://man7.org/linux/man-pages/man1/perf-report.1.html), and [kernel perf security](https://www.kernel.org/doc/html/latest/admin-guide/perf-security.html)
- Apple [Analyzing CPU profile data](https://developer.apple.com/documentation/xcode/analyzing-cpu-profile-data)
- Upstream [`samply`](https://github.com/mstange/samply), [`flamegraph`](https://github.com/flamegraph-rs/flamegraph), and [Criterion](https://bheisler.github.io/criterion.rs/book/)
- [Rust Project Primer: Measuring Performance](https://rustprojectprimer.com/measure/performance.html) (starting overview)