# Tiered CI design (Phase 6)

> Status: **live.** These workflows are committed under `.github/workflows/`
> (`smoke.yml`, `pr.yml`, `nightly.yml`, `release.yml`) — those files are the
> source of truth. This document is the design rationale and tier reference.
>
> Three things were corrected versus the original draft so the committed CI is
> actually green: `smoke` runs the existing fast tests
> (`stability` / `no_alloc` / `vactrol_envelope`) rather than a non-existent
> `smoke` test; `pr`'s bench-gate is a dependency-free `cargo bench --no-run`
> compile check until a CodSpeed `criterion-compat` shim is added (then the
> instruction-count + wall-clock gate below becomes opt-in); and the fast-math
> matrix uses `-Cllvm-args=-fp-contract=fast` (the `-ffast-math` in the draft is a
> clang flag that `rustc` rejects).

The core principle (Part D of the report): **the DSP core has no VCV Rack SDK
dependency**, so the entire test and bench suite runs headless. No core job
touches the Rack SDK; the VCV adapter (Phase 7) is a separate, optional job.

Tiers:
- **smoke** (every push, fast, must-pass): build + clippy + smoke tests across
  arch x fast-math.
- **pr** (PR to main): full test suite across the matrix + benchmark gating.
- **nightly** (schedule / dispatch / label): full suite, extended spectral
  sweeps, trend tracking.
- **release** (tag): build/package the staticlib/cdylib artifacts.

The matrix covers x86-64 and aarch64, and FP contraction on/off
(`-Cllvm-args=-fp-contract=fast`): FMA contraction reorders rounding and breaks
bit-reproducibility, so both are tested. (Note: `-ffast-math` is a C/clang flag,
not a rustc flag — `rustc` rejects it; fp-contract is the stable-rustc knob.)

## `.github/workflows/smoke.yml`

```yaml
name: smoke
on: [push]
jobs:
  smoke:
    strategy:
      fail-fast: false
      matrix:
        target: [x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu]
        fastmath: ["", "-Cllvm-args=-fp-contract=fast"]
    runs-on: ubuntu-latest
    env:
      RUSTFLAGS: ${{ matrix.fastmath }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - name: Install cross linker (aarch64)
        if: matrix.target == 'aarch64-unknown-linux-gnu'
        run: sudo apt-get update && sudo apt-get install -y gcc-aarch64-linux-gnu
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo build --target ${{ matrix.target }}
      - run: cargo test --test stability --test no_alloc --test vactrol_envelope
```

## `.github/workflows/pr.yml`

```yaml
name: pr
on:
  pull_request:
    branches: [main]
jobs:
  test:
    strategy:
      fail-fast: false
      matrix:
        fastmath: ["", "-Cllvm-args=-fp-contract=fast"]
    runs-on: ubuntu-latest
    env:
      RUSTFLAGS: ${{ matrix.fastmath }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace
  bench-gate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      # Committed gate: dependency-free compile check of the benchmark target.
      - run: cargo bench --bench lpg --no-run
```

Opt-in upgrade (requires adding `codspeed-criterion-compat` to the crate and a
`CODSPEED_TOKEN` secret), giving deterministic instruction-count gating plus a
same-runner wall-clock comparison for the SIMD/DSP throughput metric:

```yaml
      - uses: CodSpeedHQ/action@v3
        with:
          run: cargo codspeed build && cargo codspeed run
          token: ${{ secrets.CODSPEED_TOKEN }}
```

## `.github/workflows/nightly.yml`

```yaml
name: nightly
on:
  schedule: [{ cron: "0 5 * * *" }]
  workflow_dispatch:
jobs:
  full:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace --release
      - run: cargo bench --bench lpg   # trend tracking via CodSpeed/Bencher
```

## `.github/workflows/release.yml`

```yaml
name: release
on:
  push:
    tags: ["v*"]
jobs:
  package:
    strategy:
      matrix:
        target: [x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: "${{ matrix.target }}" }
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: actions/upload-artifact@v4
        with:
          name: vactrol-core-${{ matrix.target }}
          path: |
            target/${{ matrix.target }}/release/libvactrol_core.a
            target/${{ matrix.target }}/release/libvactrol_core.so
```

## Reproducibility

- All imperfection RNG is seeded; tests pin seeds.
- Goldens are tolerance-compared (never bit-exact) and regenerated deliberately
  via `cargo run -p vactrol-harness -- bless`, which writes a `manifest.json`
  recording sample rate, seed, crate version, arch, and OS.
