# Polish core loose ends + commit CI — design

## North star

complib is a **testable, reliable analog-emulation component library**. This work
hardens the *reliability and testability* of the existing component (vactrol LPG)
and stands up the CI that keeps it reliable as the library grows. It does **not**
add a second component — that is a separate, later stream. The value here is the
substrate a multi-component lib needs first: an honest test suite, real CI, and a
core whose docs match its code.

## Goal (session)

> deliver what we need to build the analog component lib for building VCV Rack
> modules

That reframes the target: it is not enough to harden the headless core — the
**bridge from the core to an actual VCV Rack module must build and be verified**.
The Rack SDK is present on this machine (`~/Projects/Rack-SDK`, arm64, Rack v2),
so we build the real plugin, not just a link smoke test.

## Scope

Streams against the current `main` (vactrol-core green, 8 commits in):

- **Stream A — CI**: commit the documented workflows so reliability is enforced,
  not aspirational. Correct the two documented-but-broken bits (see A below) so
  the committed CI is actually green.
- **Stream B — Polish**: remove an inert flag that makes the suite test a lie
  (B1), reconcile stale docs (B3), and close the SIMD path's analog-fidelity gap
  (B2, the separable stream).
- **Stream C — VCV adapter builds**: take the adapter from unverified scaffold to
  a plugin that compiles, links the Rust staticlib, and loads. This is the piece
  the session goal most directly demands.

Out of scope: adding a second component; ABI stabilization; the per-sample Newton
relinearisation of the resonance nonlinearity (the instantaneous linearisation
already in `audio_path.rs` is sufficient and was the documented upgrade); exposing
the SIMD `LpgX4` path over the FFI (the adapter stays scalar-per-channel for now —
a noted follow-up).

## Background: what's actually true today

The state-space rewrite (commit `c9e7893`, "Replace audio path with the authors'
actual 292 state-space model") changed the antialiasing design but left artifacts:

- **`adaa` is a dead flag.** `Params::adaa` is set (`true`) in ~12 sites — every
  test, `reference.rs`, `ffi.rs`, `benches/lpg.rs` — but **nothing reads it**.
  `audio_path.rs` linearises the `tanh` resonance nonlinearity about the previous
  output `xo` (the paper's instantaneous approach) inside the delay-free solve;
  antialiasing comes from oversampling that whole solve. The old memoryless
  output-buffer ADAA (`src/nonlinear.rs`: `TanhAdaa`, `saturate`,
  `tanh_antiderivative`, ~90 lines) is never called anywhere in the signal path.
- **DESIGN.md is stale.** Its "Antialiasing notes" describe an "explicit one-sample
  feedback" buffer and frame instantaneous linearisation as a *future* upgrade —
  but that upgrade already shipped. `oversample.rs`'s module doc still says it
  oversamples "the memoryless buffer nonlinearity"; it actually oversamples the
  full solve via a closure.
- **SIMD has no imperfection.** `LpgX4` intentionally skips the imperfection layer
  (documented in its module header), so polyphonic voices are deterministic and
  identical — not analog-distinct.

## Stream A — Commit CI workflows

`docs/CI.md` contains four ready-to-commit YAML blocks (`smoke.yml`, `pr.yml`,
`nightly.yml`, `release.yml`) that were documented-not-committed only because the
branch-creating bot lacked the GitHub `workflows` permission. The repo owner does
not have that limit.

**Changes:**
- Create `.github/workflows/{smoke,pr,nightly,release}.yml` from the verbatim YAML
  in `docs/CI.md` (no behavioral edits; copy exactly).
- Rewrite the opening note of `docs/CI.md`: from "documented here because the bot
  lacks `workflows` permission" to "these workflows are live in
  `.github/workflows/`; this file is the rationale and tier reference."

**Two corrections to the documented YAML** (committing red CI would contradict the
"reliable" goal):
- `smoke.yml` runs `cargo test --test smoke` — there is **no `smoke.rs`** in the
  suite. Replace with real fast must-pass tests: `--test stability --test no_alloc
  --test vactrol_envelope`.
- `pr.yml`'s bench-gate uses CodSpeed (`cargo codspeed`, `CODSPEED_TOKEN`) but the
  crate has **no codspeed dependency** (only a comment promising one). Replace the
  gate with a dependency-free `cargo bench --bench lpg --no-run` (compile check),
  and note CodSpeed as an opt-in future enhancement in `docs/CI.md`.

**Verification:** parse each file with `actionlint` if available, else a YAML
parse check (`python3 -c "import yaml; yaml.safe_load(open(p))"` per file). Tiers:
smoke (every push), pr (full matrix + bench compile), nightly (extended sweeps),
release (artifact build).

## Stream B1 — Remove the inert ADAA machinery

The flag does nothing; keeping it is a correctness trap (the suite asserts a
config that has no effect). Decision (approved): **remove**, do not re-wire.

**Changes:**
- Delete `crates/vactrol-core/src/nonlinear.rs` and its `pub mod nonlinear;` line
  in `lib.rs`.
- Remove `adaa` from `Params` and from `Params::Default`.
- Update `ffi.rs`: drop the `adaa` parameter from the C entry point that takes it
  (`vactrol_set_params` or equivalent), and regenerate / hand-edit
  `vcv-adapter/vactrol_core.h` (and `cbindgen.toml` output) so the header matches.
- Scrub the ~12 sites that set `adaa:` — tests (`simd`, `frequency_response`,
  `vactrol_envelope`, `imperfection`, `resonance`, `spectral`, `stability`,
  `no_alloc`), `reference.rs`, and `benches/lpg.rs`. In the bench, the
  `1x_noadaa` / `1x_adaa` / `2x_adaa` / `4x_adaa` case list collapses to vary
  **oversample factor only** (`1x` / `2x` / `4x`); relabel accordingly.
- `SerializedState` embeds `Params`. Serde ignores unknown fields by default, so
  old presets carrying an `adaa` key still deserialize — **verify this with a
  round-trip test of a JSON blob that includes a stray `adaa` field**, do not just
  assume it.

**Invariant:** removing an inert flag must not change any deterministic output.
The golden buffers must be byte-for-byte unchanged (within existing tolerance). If
a golden test shifts, that is a bug in the change, not a reason to re-bless.

## Stream B2 — Per-lane distinct imperfection in `LpgX4` (separable)

Decision (approved): **per-lane distinct** — each of the four voices is its own
slightly-different physical channel, the realistic analog behavior. This is the
one stream that can be cut at plan time without weakening the foundation; it adds
emulation fidelity and test coverage rather than reliability of what exists.

**Approach — reuse the scalar layer for guaranteed parity:**
- `LpgX4` holds `[Imperfection; 4]`, reusing the exact scalar `Imperfection` code
  (no re-derivation on the vector path → lane *i* provably mirrors a scalar `Lpg`
  with the same seed).
- Seeds derived deterministically from one base seed via a fixed per-lane salt
  (e.g. `base ^ LANE_SALT[i]`), so a 4-voice instance is reproducible and
  serializable from a single seed.
- Lift the per-lane-varying quantities to `f32x4` in the SIMD solve:
  - **components** (`c1/c2/c3`, vactrol law `A/B`, `r_on_min/r_off`, `tau_*`) from
    each lane's `tolerance_components(&base_comp)`;
  - **resonance** and **cv_offset**, which `apply_params` perturbs per lane via
    drift/thermal.
- Per sample: `update(sample_rate)` each lane; gather per-lane params/components
  into the `f32x4` operating point; run the existing vector solve; then
  `apply_output` per lane on the output `f32x4` (gain drift + pink noise floor).
- Gather/scatter between `[T; 4]` and `f32x4` at the lane boundary is acceptable;
  the hot vector arithmetic (the solve) stays vectorized. Imperfection **off** must
  keep the current splat fast path (no per-lane gather cost when disabled).

**Tests (extend `tests/simd.rs`):**
- Imperfection **off**: SIMD still matches scalar within the existing `1e-3`
  tolerance (no regression).
- Imperfection **on**: each lane matches its scalar `Lpg` counterpart driven with
  that lane's derived seed, within `1e-3`.
- Determinism: same base seed → same four-lane output across runs.

## Stream B3 — Doc reconciliation

- Rewrite DESIGN.md "Antialiasing notes" and the Phase 2 bullet to describe the
  real design: in-loop instantaneous linearisation of the resonance `tanh` about
  `xo`, antialiased by oversampling the full delay-free solve (1x/2x/4x); the
  memoryless ADAA buffer has been removed. Delete the "tighten the loop = future
  upgrade" paragraph (already shipped).
- Update the DESIGN.md SIMD status line: imperfection now applies on the SIMD path
  (per-lane distinct), if B2 ships.
- Fix `oversample.rs` module doc: "oversamples the full delay-free solve," not
  "the memoryless buffer nonlinearity."

## Stream C — VCV adapter actually builds

The adapter (`vcv-adapter/`) is a faithful but never-compiled skeleton. The
session goal requires it to build against the present Rack SDK.

**Changes:**
- **ABI sync (depends on B1):** regenerate `vcv-adapter/vactrol_core.h` with
  `cbindgen` after `adaa` is removed (`cd crates/vactrol-core && cbindgen --config
  cbindgen.toml --output ../../vcv-adapter/vactrol_core.h`). Update
  `src/VactrolLPG.cpp`'s `vactrol_lpg_set_params(...)` call to drop the trailing
  `adaa` argument (currently passes `1`).
- **Panel:** add `vcv-adapter/res/VactrolLPG.svg` — a minimal valid Rack panel
  (correct HP width, mm units) so the module loads and `make` packages it. The
  widget already places knobs/ports at fixed mm coordinates; the SVG must match
  that footprint.
- **Build & verify:** `cd vcv-adapter && make RACK_DIR=~/Projects/Rack-SDK`
  builds `plugin.dylib` linking `../target/release/libvactrol_core.a`. Success
  criterion: clean compile + link (the Rust-in-Rack linking risk the README flags
  is retired on this arm64/macOS target). A loaded-in-Rack runtime check is
  out of scope (no scripted Rack host), but the build+link is the deliverable.

**Note:** Cargo must emit the staticlib for the right arch; the host is arm64 and
the SDK is arm64, so the default target is correct.

## Verification gate (whole change)

- `cargo test` — all suites green, **golden buffers unchanged**.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo bench --bench lpg` — builds and runs (smoke; relabeled cases present).
- `cargo run -p vactrol-harness -- gen` — still produces a pluck.
- CI YAML parses.

## Build sequence

1. Stream A (CI) — independent, lowest risk, lands reliability infra first.
2. B1 (remove ADAA) — touches the most files (incl. FFI + C++ caller); do before
   C so the regenerated header reflects the final ABI, and before B3 so docs
   describe the post-removal state.
3. Stream C (VCV plugin builds) — depends on B1's ABI; end-to-end proof of the
   session goal. Regenerate header, fix caller, add panel, `make`.
4. B3 (docs) — reflects B1's result, B2's status, and the now-working adapter.
5. B2 (SIMD imperfection) — largest DSP change; isolated to `simd.rs` + its test.
   Separable: can be deferred to a follow-up if priorities shift to component #2.
