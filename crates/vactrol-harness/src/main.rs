//! Headless harness: signal generation and (later) golden management.
//!
//! Usage:
//!   vactrol-harness gen     Run a demo pluck and print stats.
//!   vactrol-harness bless   Regenerate golden buffers (implemented in Phase 4).

use vactrol_core::{Lpg, Mode, Params};

fn main() {
    let cmd = std::env::args().nth(1).unwrap_or_else(|| "gen".to_string());
    match cmd.as_str() {
        "gen" => gen(),
        "bless" => bless(),
        other => {
            eprintln!("unknown command: {other}");
            eprintln!("commands: gen, bless");
            std::process::exit(2);
        }
    }
}

/// Drive a short CV gate into a 220 Hz tone and report the resulting pluck.
fn gen() {
    let sr = 48_000.0f32;
    let mut lpg = Lpg::new(sr);
    lpg.set_params(Params {
        mode: Mode::Both,
        resonance: 0.2,
        cv_offset: 0.0,
        drive: 1.0,
    });

    let n = (sr * 0.5) as usize; // 500 ms
    let gate_samples = (sr * 0.005) as usize; // 5 ms gate
    let two_pi = std::f32::consts::TAU;

    let mut peak = 0.0f32;
    let mut peak_idx = 0usize;
    for i in 0..n {
        let cv = if i < gate_samples { 8.0 } else { 0.0 };
        let audio = (two_pi * 220.0 * i as f32 / sr).sin();
        let y = lpg.process_sample(audio, cv);
        if y.abs() > peak {
            peak = y.abs();
            peak_idx = i;
        }
    }

    println!(
        "samples={n} peak={peak:.4} peak_idx={peak_idx} ({:.1} ms)",
        peak_idx as f32 / sr * 1000.0
    );
    println!("final_rf={:.0} ohm", lpg.last_rf());
}

fn bless() {
    println!("golden bless lands in Phase 4 (test harness); nothing to do yet");
}
