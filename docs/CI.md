# Tiered CI design (Phase 6)

> Note: the workflow YAML below is ready to drop into `.github/workflows/`, but it
> is documented here rather than committed because the automation that created
> this branch lacks the GitHub `workflows` permission and cannot push files under
> `.github/workflows/`. Add these from an account/app with that permission.

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

The matrix covers x86-64 and aarch64, and `-ffast-math` on/off (fast-math
reordering breaks denormal handling and bit-reproducibility, so both are tested).

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
        fastmath: ["", "-Cllvm-args=-fp-contract=fast -ffast-math"]
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
      - run: cargo test --test smoke --test stability --test no_alloc
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
        fastmath: ["", "-Cllvm-args=-fp-contract=fast -ffast-math"]
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
      # CodSpeed simulation mode: deterministic instruction-count gating.
      - uses: CodSpeedHQ/action@v3
        with:
          run: cargo codspeed build && cargo codspeed run
          token: ${{ secrets.CODSPEED_TOKEN }}
      # Plus a same-runner wall-clock relative comparison (base vs PR) for the
      # throughput metric that instruction count cannot capture for SIMD/DSP.
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
