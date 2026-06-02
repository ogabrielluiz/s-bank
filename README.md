# complib

A portable virtual-analog DSP component library. The first slice is a
vactrol low-pass gate (Buchla 292 style): a topology-preserving 2-pole core
driven by an asymmetric vactrol envelope, built as a headless, testable Rust
core that a thin C++ VCV adapter can link over a C ABI.

- DSP core: [`crates/vactrol-core`](crates/vactrol-core)
- Headless harness: [`crates/vactrol-harness`](crates/vactrol-harness)
- Design notes and references: [`docs/DESIGN.md`](docs/DESIGN.md)
- CI design (Phase 6): [`docs/CI.md`](docs/CI.md)

The core is a TPT/ZDF 2-pole audio path, an asymmetric vactrol envelope,
first-order ADAA + polyphase halfband oversampling (1x/2x/4x) on the buffer
nonlinearity, and a seedable/serializable analogue imperfection layer
(per-instance tolerance, drift, thermal wander, noise floor).

## Quick start

```sh
cargo test                            # smoke, correctness, spectral, golden
cargo clippy --all-targets -- -D warnings
cargo bench --bench lpg               # per-config / voices / worst-case
cargo run -p vactrol-harness -- gen   # demo pluck
cargo run -p vactrol-harness -- bless # regenerate golden buffers
```
