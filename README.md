# complib

A portable virtual-analog DSP component library. The first slice is a
vactrol low-pass gate (Buchla 292 style): a topology-preserving 2-pole core
driven by an asymmetric vactrol envelope, built as a headless, testable Rust
core that a thin C++ VCV adapter can link over a C ABI.

- DSP core: [`crates/vactrol-core`](crates/vactrol-core)
- Headless harness: [`crates/vactrol-harness`](crates/vactrol-harness)
- Design notes and references: [`docs/DESIGN.md`](docs/DESIGN.md)

## Quick start

```sh
cargo test                            # smoke + correctness + spectral tests
cargo clippy --all-targets -- -D warnings
cargo run -p vactrol-harness -- gen   # demo pluck
```
