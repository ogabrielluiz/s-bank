//! Minimal benchmark scaffold (Phase 5 expands this into ns/sample, 1-vs-16-voice
//! SIMD scaling, per-oversampling-factor, ADAA on/off, and worst-case inputs,
//! imported as `criterion` and backed by `codspeed-criterion-compat`).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vactrol_core::{Lpg, Mode, Params};

fn bench_process_block(c: &mut Criterion) {
    let mut lpg = Lpg::new(48_000.0);
    lpg.set_params(Params {
        mode: Mode::Both,
        resonance: 0.5,
        cv_offset: 0.0,
        drive: 1.0,
    });

    let audio: Vec<f32> = (0..512).map(|i| (i as f32 * 0.1).sin()).collect();
    let cv = vec![5.0f32; 512];
    let mut out = vec![0.0f32; 512];

    c.bench_function("process_block_512", |b| {
        b.iter(|| lpg.process_block(black_box(&audio), black_box(&cv), &mut out));
    });
}

criterion_group!(benches, bench_process_block);
criterion_main!(benches);
