//! Benchmark suite. Imported as `criterion`; under `cargo codspeed` this is the
//! `codspeed-criterion-compat` shim (Phase 6 wires that in CI). Throughput is set
//! per element so Criterion reports time-per-sample directly.
//!
//! Coverage: per-oversampling-factor cost; imperfection on/off; 1 vs 16 voices
//! (scalar vs the lane-parametric SIMD path); and an explicit worst-case input
//! alongside the typical case, since the real-time guarantee is about the worst
//! case.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use vactrol_core::{ImperfectionConfig, Lpg, LpgX4, Mode, Params};
use wide::f32x4;

const SR: f32 = 48_000.0;
const BLOCK: usize = 512;

fn lpg(oversample: u8, imperfection: bool, drive: f32) -> Lpg {
    let mut l = Lpg::new(SR);
    l.set_params(Params {
        mode: Mode::Both,
        resonance: 0.5,
        cv_offset: 0.0,
        drive,
        oversample,
    });
    if imperfection {
        l.set_imperfection(ImperfectionConfig {
            enabled: true,
            ..Default::default()
        });
    }
    l
}

/// A musically typical input: a tone with an occasional gate.
fn typical_input() -> (Vec<f32>, Vec<f32>) {
    let audio: Vec<f32> = (0..BLOCK)
        .map(|i| (std::f32::consts::TAU * 220.0 * i as f32 / SR).sin())
        .collect();
    let cv: Vec<f32> = (0..BLOCK).map(|i| if i < 16 { 8.0 } else { 0.0 }).collect();
    (audio, cv)
}

/// Worst case: full-scale rapidly-alternating audio (maximizes the nonlinear work
/// in the resonance solve) with audio-rate CV slamming the gate, so every sample
/// does the full tanh-linearised filter solve.
fn worst_input() -> (Vec<f32>, Vec<f32>) {
    let audio: Vec<f32> = (0..BLOCK)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let cv: Vec<f32> = (0..BLOCK)
        .map(|i| 10.0 * (std::f32::consts::TAU * 8_000.0 * i as f32 / SR).sin())
        .collect();
    (audio, cv)
}

fn bench_configs(c: &mut Criterion) {
    let mut g = c.benchmark_group("config");
    g.throughput(Throughput::Elements(BLOCK as u64));
    let (audio, cv) = typical_input();
    let mut out = vec![0.0f32; BLOCK];
    for (name, os) in [("1x", 1u8), ("2x", 2), ("4x", 4)] {
        let mut l = lpg(os, false, 2.0);
        g.bench_function(name, |b| {
            b.iter(|| l.process_block(black_box(&audio), black_box(&cv), &mut out))
        });
    }
    g.finish();
}

fn bench_imperfection(c: &mut Criterion) {
    let mut g = c.benchmark_group("imperfection");
    g.throughput(Throughput::Elements(BLOCK as u64));
    let (audio, cv) = typical_input();
    let mut out = vec![0.0f32; BLOCK];
    for (name, imp) in [("off", false), ("on", true)] {
        let mut l = lpg(2, imp, 2.0);
        g.bench_function(name, |b| {
            b.iter(|| l.process_block(black_box(&audio), black_box(&cv), &mut out))
        });
    }
    g.finish();
}

fn bench_voices(c: &mut Criterion) {
    let mut g = c.benchmark_group("voices");
    let (audio, cv) = typical_input();
    let mut out = vec![0.0f32; BLOCK];

    g.throughput(Throughput::Elements(BLOCK as u64));
    let mut one = lpg(2, false, 2.0);
    g.bench_function("x1", |b| {
        b.iter(|| one.process_block(black_box(&audio), black_box(&cv), &mut out))
    });

    g.throughput(Throughput::Elements(16 * BLOCK as u64));
    let mut many: Vec<Lpg> = (0..16).map(|_| lpg(2, false, 2.0)).collect();
    g.bench_function("x16_scalar", |b| {
        b.iter(|| {
            for l in many.iter_mut() {
                l.process_block(black_box(&audio), black_box(&cv), &mut out);
            }
        })
    });

    // 16 voices as four SIMD blocks of 4 lanes each.
    let audio4: Vec<f32x4> = audio.iter().map(|&a| f32x4::splat(a)).collect();
    let cv4: Vec<f32x4> = cv.iter().map(|&c| f32x4::splat(c)).collect();
    let mut out4 = vec![f32x4::splat(0.0); BLOCK];
    let mut blocks: Vec<LpgX4> = (0..4)
        .map(|_| {
            let mut l = LpgX4::new(SR);
            l.set_params(Params {
                mode: Mode::Both,
                resonance: 0.5,
                cv_offset: 0.0,
                drive: 2.0,
                oversample: 2,
            });
            l
        })
        .collect();
    g.bench_function("x16_simd", |b| {
        b.iter(|| {
            for l in blocks.iter_mut() {
                l.process_block(black_box(&audio4), black_box(&cv4), &mut out4);
            }
        })
    });
    g.finish();
}

fn bench_worst_vs_typical(c: &mut Criterion) {
    let mut g = c.benchmark_group("worst_vs_typical");
    g.throughput(Throughput::Elements(BLOCK as u64));
    let mut out = vec![0.0f32; BLOCK];

    let (ta, tc) = typical_input();
    let mut l1 = lpg(4, true, 4.0);
    g.bench_function("typical_4x_imperf", |b| {
        b.iter(|| l1.process_block(black_box(&ta), black_box(&tc), &mut out))
    });

    let (wa, wc) = worst_input();
    let mut l2 = lpg(4, true, 4.0);
    g.bench_function("worst_4x_imperf", |b| {
        b.iter(|| l2.process_block(black_box(&wa), black_box(&wc), &mut out))
    });
    g.finish();
}

criterion_group!(
    benches,
    bench_configs,
    bench_imperfection,
    bench_voices,
    bench_worst_vs_typical
);
criterion_main!(benches);
