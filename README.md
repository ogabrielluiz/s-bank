# S-Bank

**The signal bank** — a library of analog-emulation DSP building blocks that help you
build VCV Rack modules with convincing analog behaviour and sound. Part of the
**Sam-e** signal system (`S-` is the signal). The library is the product; the modules
here are demos that use it to prove it works and show how.

Each component is a headless, testable Rust core; a thin C++ VCV Rack adapter links
them over a single C ABI. Two instruments live in the bank today: a **vactrol
low-pass gate** (Buchla 292 style — dirty, resonant) and **Strike**, a clean,
zero-bleed, envelope-driven low-pass gate.

## Repo layout — the library vs. the demos

- **`components/`** — the library: reusable analog-emulation DSP cores (pure Rust
  rlibs, no Rack dependency).
  - [`vactrol-core`](components/vactrol-core) — Buchla-292 vactrol LPG.
  - [`strike-core`](components/strike-core) — clean EG-driven gate.
- **`modules/`** — demo VCV Rack modules built on the library:
  - [`plugin`](modules/plugin) — one Rust staticlib re-exporting every component over a
    flat C ABI (so they share one runtime in the plugin).
  - [`rack`](modules/rack) — the C++ VCV adapter: module sources, panels, `plugin.json`.
- **`tools/`** — [`harness`](tools/harness) (headless gen/bless).
- **`site/`** — the Sam-e / S- brand living document.
- **`docs/`** — design notes ([`DESIGN.md`](docs/DESIGN.md)) and CI design ([`CI.md`](docs/CI.md)).

## Quick start

```sh
cargo test                                 # all components: smoke, golden, spectral, behavior
cargo clippy --all-targets -- -D warnings
cargo run -p vactrol-harness -- gen        # demo pluck (vactrol)

# VCV Rack plugin (needs the Rack SDK) — builds modules/plugin then the adapter:
cd modules/rack && make install RACK_DIR=/path/to/Rack-SDK
```
