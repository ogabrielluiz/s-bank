# S-Bank Strike — clean EG-driven low-pass gate — design

## What & why

**Strike** is the second instrument in S-Bank: a **clean, zero-bleed, envelope-driven
low-pass gate**, in the spirit of the well-known "natural"-style dual LPG that
deliberately avoids vactrols (verified: the reference fully closes / zero bleed and
"decided against the vactrols used by Buchla"). Its character lives in a **carefully
shaped envelope generator**, not in component physics. It is an independent design —
our own engine, name, panel, and code; nothing references the reference module.

Sibling to `vactrol-core` (the dirty/resonant 292 vactrol LPG). Together the bank now
offers two complementary gates: vactrol (bleed + resonance + drift) and Strike (clean,
full-closing, shaped).

## Signature behaviors to reproduce (the sound)

1. **Zero bleed / full close** — at control 0, both cutoff → floor and VCA gain → 0;
   no leakthrough. (Opposite of the vactrol core.)
2. **More filter than VCA** — one control sweeps cutoff *and* amplitude together, with
   the low-pass dominating the loudness change; highs open/close first.
3. **Dual-mode decay** — two distinct envelope shapes (short = percussive click/tap;
   long = multi-second ring), blended by the DECAY control rather than one stretched
   curve. Short = sharp; long = high energy up front then a gradual tail.
4. **Frequency-dependent decay** — higher-pitched input rings shorter, lower rings
   longer (always-on). Estimate input pitch/brightness cheaply (smoothed zero-crossing
   rate or a 1-pole brightness follower) and scale decay inversely. Patching DECAY CV
   can compensate.
5. **Memory effect** — the EG does **not** reset on retrigger; a new HIT adds to the
   still-decaying envelope so rapid strikes accumulate and open the gate progressively
   wider.
6. **Ping / EG-out** — `IN` is normalled to a DC level; a HIT with `IN` unpatched emits
   the raw envelope (0..+10 V unipolar) at `OUT` (clean ping + usable as a CV source).
7. **CTRL pre-EG, OPEN post-EG** — CTRL adds opening before the EG (so DECAY scales its
   effect), normalled to +10 V, negative-clamped; OPEN is a post-EG floor the gate never
   closes below.
8. **No resonance** — fixed low Q; the filter character comes from the swept cutoff +
   MATERIAL, not resonance. (Our resonance lives in the vactrol core; Strike stays clean.)

## Improvements (what makes it ours, approved)

- **Continuous MATERIAL morph** — replace the reference's fixed 3-position hard/med/soft
  switch with a continuous `MATERIAL` knob (+ CV): morphs attack speed, brightness
  ceiling, and output level along a hard→soft axis. Optional named zones
  (hard/medium/soft) marked on the panel as reference points, but the control is smooth.
- **Optional analog-imperfection** — a switch that engages the existing
  `imperfection` layer (per-instance tolerance, drift, thermal wander, noise floor) on
  Strike's decay/cutoff/level, so the clean engine can be dirtied on demand. Off by
  default (bit-clean). Reuses `vactrol-core`'s seedable `Imperfection`.

(Explicitly *not* in scope this pass, per the chosen options: resonance/ring mode,
stereo-sum/true-poly link. Easy to add later.)

## Architecture

New crate **`crates/strike-core`** mirroring the `vactrol-core` shape (headless,
testable, no Rack SDK), reusing shared DSP:

- Depend on `vactrol-core` (path dep) to reuse `oversample` (polyphase halfband) and
  `imperfection` (the optional-imperfection improvement) rather than duplicating them.
  *(Future cleanup as the bank grows: factor a shared `s-bank-dsp` crate; out of scope
  now.)*

Modules in `strike-core`:
- `envelope.rs` — the shaped EG: trigger handling (HIT threshold +0.25 V → shaped drive
  pulse), dual-mode decay curves, memory (no-reset accumulation), frequency-dependent
  decay scaling, OPEN floor (post), CTRL add (pre, clamped). Outputs a 0..1 control.
- `material.rs` — the continuous morph: maps `material ∈ [0,1]` → (attack_time,
  cutoff_ceiling, level). Hard (0) = fast/bright/loud; soft (1) = slow/dull/quieter.
- `gate.rs` — the clean zero-bleed LPG cell: TPT/ZDF 2-pole low-pass, cutoff driven by
  control × material ceiling, plus a co-modulated VCA gain (filter-dominant). Fixed low
  Q, full close at control 0. Oversampled via the shared `oversample` for clean
  audio-rate CTRL/ping.
- `pitch.rs` — cheap input pitch/brightness estimator for freq-dependent decay.
- `params.rs` — `StrikeParams` (open, decay, decay_cv_atten, material, ctrl_atten,
  imperfection on/off, seed, oversample) + `Components`-style tunables.
- `strike.rs` / `lib.rs` — `Strike` voice tying it together; `process_sample(audio_in,
  ctrl_in, decay_cv, hit) -> f32` with `IN`-normalling for ping; block + (later) SIMD.
- `ffi.rs` — C ABI (`strike_create/destroy/set_sample_rate/set_params/process_*`),
  cbindgen header.

Signal flow per sample:
```
HIT ─▶ EG (shaped, memory, freq-dep decay) ─┐
CTRL (pre, clamp, atten) ───────────────────┤─▶ control 0..1 ─▶ gate cell (cutoff+VCA)
OPEN (post floor) ──────────────────────────┘                         ▲
DECAY (slider + CV·atten) ─▶ EG decay-time pivot          MATERIAL ───┘ (ceiling/attack/level)
IN (audio; normalled to DC when unpatched) ─────────────▶ gate cell ─▶ OUT
[optional imperfection layer perturbs decay/cutoff/level]
```

## VCV module — "Strike" (dual channel)

Two independent channels (mirrored), each poly-capable (per-channel loop like the
vactrol module). Per channel:
- Controls: `OPEN`, `DECAY`, `MATERIAL` knobs; `DECAY` CV attenuverter; `CTRL`
  attenuverter; (global) `IMPERFECTION` switch + an `OS` (oversample) switch.
- Jacks: `IN`, `HIT`, `DECAY` (CV), `CTRL` (CV, normalled +10 V), `OUT`.
- Light: per-channel openness LED (brightness = gate opening). Optional small scope.
- Panel: our own SVG in the Sam-e visual language (crosshairs, serials, S- mark,
  signal-state LED) — ~12–16 HP, two mirrored channels. No copied artwork.
- `plugin.json`: add a second module `slug: "Strike"`, brand stays `S-Bank`.

## Tests (TDD)

`strike-core` behavior tests (pin seeds; tolerance-compared):
- **zero-bleed**: control 0 ⇒ output is silence (≤ −120 dB) with audio at IN (the one
  hard contrast vs the vactrol core, which bleeds).
- **ping/EG-out**: IN unpatched (normalled) + HIT ⇒ OUT emits a unipolar 0..~+10 V
  envelope matching the EG shape.
- **decay length**: DECAY low ⇒ short (percussive) tail; DECAY high ⇒ long tail;
  monotonic with the control; the two regimes differ in shape, not just scale.
- **memory effect**: N rapid HITs ⇒ peak opening increases vs a single HIT (accumulation).
- **frequency-dependent decay**: high-pitched IN decays measurably faster than low.
- **more-filter-than-VCA**: as control opens, spectral centroid rises (brightness opens),
  and loudness change tracks the filter more than a pure VCA would.
- **material morph**: material 0→1 monotonically slows attack, lowers cutoff ceiling, and
  drops level; continuous (no zipper).
- **imperfection off = deterministic**; on = bounded per-instance variation (seeded).
- **no_alloc** on the audio path; **stability** under pathological input; golden buffers
  via the harness pattern.

## Verification gate

`cargo test` (incl. new strike tests + existing vactrol goldens unchanged), `cargo
clippy --all-targets -- -D warnings`, `cargo bench` builds, then build + `make install`
the VCV plugin and confirm **both** modules load in Rack (`log.txt`: "Loaded plugin
SBankVactrol 2.0.0" with Strike registered), and patch Strike to hear a struck ring.

## Build sequence

1. `strike-core` scaffold + `params` + clean `gate` cell (zero-bleed) — test zero-bleed.
2. `envelope` (dual-mode decay, OPEN/CTRL, ping) — test decay length + ping + memory.
3. `pitch` + frequency-dependent decay — test.
4. `material` continuous morph — test.
5. Optional `imperfection` wiring — test off=deterministic / on=bounded.
6. `ffi` + cbindgen header.
7. VCV `Strike` module + panel + plugin.json; build + install + verify in Rack.
8. SIMD `StrikeX4` (later, mirrors vactrol pattern) — optional follow-up.
