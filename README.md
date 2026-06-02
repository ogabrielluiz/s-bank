# S-Bank

**The signal bank** — a virtual-analog DSP component library for VCV Rack. Part of
the **Sam-e** signal system (`S-` is the signal); `S-Bank` is the growing bank of
audio instruments built on it.

Each component is a headless, testable Rust core that a thin C++ VCV Rack adapter
links over a C ABI. The first instrument in the bank is a **vactrol low-pass gate**
(Buchla 292 style): a topology-preserving 2-pole core driven by an asymmetric
vactrol envelope.

- DSP core: [`crates/vactrol-core`](crates/vactrol-core)
- Headless harness: [`crates/vactrol-harness`](crates/vactrol-harness)
- VCV Rack adapter: [`vcv-adapter`](vcv-adapter)
- Brand living document (Sam-e / S-): [`site`](site)
- Design notes and references: [`docs/DESIGN.md`](docs/DESIGN.md)
- CI design: [`docs/CI.md`](docs/CI.md)

The vactrol core is the Parker & D'Angelo 3-capacitor state-space 292 model
discretised topology-preservingly and solved as a delay-free loop each sample; the
resonance `tanh` is linearised in-loop about the previous output, with polyphase
halfband oversampling (1x/2x/4x) of the whole solve for antialiasing, a 4-voice SIMD
path, and a seedable/serializable analogue-imperfection layer (per-instance
tolerance, drift, thermal wander, noise floor — applied per lane on the SIMD path).

## Quick start

```sh
cargo test                            # smoke, correctness, spectral, golden
cargo clippy --all-targets -- -D warnings
cargo bench --bench lpg               # per-config / voices / worst-case
cargo run -p vactrol-harness -- gen   # demo pluck
cargo run -p vactrol-harness -- bless # regenerate golden buffers

# VCV Rack plugin (needs the Rack SDK):
cargo build --release -p vactrol-core
(cd vcv-adapter && make RACK_DIR=/path/to/Rack-SDK)
```
