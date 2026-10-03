# Rust Test Coverage Tooling

Research for [Research Rust test coverage tooling](https://github.com/mwilsoncoding/dozenal-music-engraving-renderer/issues/54). This report does not change the test suite or adopt a CI policy.

## Recommendation

**Adopt `cargo llvm-cov` as an optional local coverage tool; defer a CI gate and branch-coverage requirement.** This binary has meaningful parser and renderer decision paths, a growing CLI integration suite, and no project dependencies. A local line/region report can show untested behavior without changing `Cargo.toml` or the shipped binary. First inspect the baseline and use it to target tests; set a threshold only after the team chooses what source is in scope and can justify the number. Rust's compiler instrumentation is stable, but `cargo llvm-cov --branch` is explicitly unstable and is not needed for the first pass. [README](../../README.md), [MVP issue #21](https://github.com/mwilsoncoding/dozenal-music-engraving-renderer/issues/21), [rustc coverage guide](https://doc.rust-lang.org/rustc/instrument-coverage.html), [`cargo llvm-cov` options](https://github.com/taiki-e/cargo-llvm-cov#options)

## Fit to This Project

The project is a single binary: private unit tests live in `src/main.rs`, which includes the custom parser as `mod xml`; `tests/cli_to_svg.rs` launches the compiled CLI in subprocesses and exercises parsing through rendered output. Source-based instrumentation can cover the unit-test binary and the separately run CLI binary. The profile runtime writes data when each instrumented process exits; the `LLVM_PROFILE_FILE` `%p`/`%m` patterns avoid subprocesses overwriting one another, and LLVM merges the resulting profiles. Keep the environment intact for the test's `Command` child, as these integration tests currently do. This combination can measure parser code in both unit and end-to-end paths, as well as argument handling, error paths, and rendering in `src/main.rs`. [Cargo manifest](../../Cargo.toml), [unit tests and parser module](../../src/main.rs), [CLI integration tests](../../tests/cli_to_svg.rs), [rustc profile-file documentation](https://doc.rust-lang.org/rustc/instrument-coverage.html#running-the-instrumented-program), [Cargo integration-test targets](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#integration-tests)

LLVM source-based coverage embeds source-region mappings in instrumented binaries and records execution counters. `llvm-cov` reports function, instantiation, line, and region coverage; regions provide more detail than line percentages when a line contains multiple executable expressions. Rust's mapping includes generic and macro-generated function instantiations, so those can affect totals; `cargo llvm-cov` also documents cases such as cross-compiled build scripts and procedural macros that Cargo does not instrument. This project has a handwritten parser rather than generated parser code, so source attribution should be comparatively straightforward. Coverage still measures execution, not assertion quality or correctness. [rustc coverage guide](https://doc.rust-lang.org/rustc/instrument-coverage.html#creating-coverage-reports), [`cargo llvm-cov` options](https://github.com/taiki-e/cargo-llvm-cov#options)

## Options

| Workflow | Coverage and collection | Cost and fit |
| --- | --- | --- |
| **Rust's documented rustc + LLVM workflow** | Compile with `-C instrument-coverage`; run tests/programs; merge `.profraw` files with `llvm-profdata`; report/show with `llvm-cov`. Gives source-based function, instantiation, line, and region reports. Branch-level reporting is a separate unstable option. | No project crate dependency, but requires scripting profile paths, locating all relevant binaries, and matching LLVM tools to the compiler. `llvm-tools-preview` is rustup-installable; Rust notes its tool interface is not under the usual stability guarantees and raw profiles may require the compiler-matched LLVM version. Best when maximum control is needed, not the smallest first workflow. [Rust guide](https://doc.rust-lang.org/rustc/instrument-coverage.html), [LLVM `llvm-profdata`](https://llvm.org/docs/CommandGuide/llvm-profdata.html), [LLVM `llvm-cov`](https://llvm.org/docs/CommandGuide/llvm-cov.html) |
| **`cargo llvm-cov`** | Wraps the source-based LLVM flow and runs Cargo tests, including integration tests; emits text/HTML and machine-readable reports. Supports line, function, and region failure thresholds. Its `--branch` mode is marked unstable. | Recommended. Install as a separate developer/CI executable (for example, `cargo install cargo-llvm-cov`); it does not add a dependency to this project's manifest or runtime. It is still third-party tooling, with its own install/update lifecycle. Pin the Rust toolchain in CI and install the matching LLVM tools. [Project README](https://github.com/taiki-e/cargo-llvm-cov), [options and thresholds](https://github.com/taiki-e/cargo-llvm-cov#options), [Rust guide](https://doc.rust-lang.org/rustc/instrument-coverage.html#installing-llvm-coverage-tools) |
| **Mozilla `grcov`** | A report generator, not a test runner: collect Rust `.profraw` data with source-based instrumentation, then use grcov to emit formats such as HTML or LCOV. It accepts branch information when available. | More manual profile/binary-path configuration than `cargo llvm-cov`; useful when aggregating artifacts or standardizing reports across languages. It is a separate executable, available as a release or through `cargo install`, not a project dependency. Its README lists Linux, macOS, and Windows support. [grcov README](https://github.com/mozilla/grcov#example-how-to-generate-source-based-coverage-for-a-rust-project) |

There is no need to adopt both wrappers. The Rust-owned workflow is the underlying, scriptable mechanism; `cargo llvm-cov` is the more ergonomic Rust/Cargo interface for this repository. The [Rust Project Primer coverage page](https://rustprojectprimer.com/measure/coverage.html) is a useful overview of these tools, but tool-specific claims above are grounded in their owning Rust, LLVM, and project documentation.

## Smallest Useful Rollout

1. Install `cargo-llvm-cov` outside the project manifest and run `cargo llvm-cov --html` on the current suite with the toolchain's compatible LLVM component.
2. Review uncovered executable regions in `src/main.rs`/`src/xml.rs`; add only high-value tests for parser and CLI error branches that matter to the accepted MusicXML profile. Preserve the existing semantic assertions rather than optimizing for a percentage.
3. If the baseline proves useful, add a stable-toolchain CI report. Start without a hard threshold; consider a line/region floor after establishing source filters and an agreed baseline. Keep unstable branch coverage out of the MVP gate.

## Primary Sources

- [Rust compiler source-based coverage guide](https://doc.rust-lang.org/rustc/instrument-coverage.html)
- [Rust 1.60 release notes: source-based coverage](https://blog.rust-lang.org/2022/04/07/Rust-1.60.0.html#source-based-code-coverage-based-on-llvm)
- [LLVM `llvm-profdata` command guide](https://llvm.org/docs/CommandGuide/llvm-profdata.html)
- [LLVM `llvm-cov` command guide](https://llvm.org/docs/CommandGuide/llvm-cov.html)
- [`cargo llvm-cov` README and options](https://github.com/taiki-e/cargo-llvm-cov)
- [Mozilla grcov README](https://github.com/mozilla/grcov)
- [Cargo integration-test target documentation](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#integration-tests)