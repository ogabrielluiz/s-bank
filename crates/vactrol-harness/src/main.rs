//! Headless harness: signal generation and golden management.
//!
//! Usage:
//!   vactrol-harness gen     Run a demo pluck and print stats.
//!   vactrol-harness bless   (Re)generate golden buffers + manifest under testdata/.
//!
//! `bless` is the deliberate, reviewable golden-update path. It never runs as part
//! of `cargo test`; goldens are committed and compared with tolerance.

use std::path::Path;

use vactrol_core::reference;
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
        oversample: 2,
    });

    let n = (sr * 0.5) as usize; // 500 ms
                                 // Gate longer than the vactrol attack so it opens before it starts to decay.
    let gate_samples = (sr * 0.03) as usize; // 30 ms gate
    let two_pi = std::f32::consts::TAU;

    let mut peak = 0.0f32;
    let mut peak_idx = 0usize;
    for i in 0..n {
        let cv = if i < gate_samples { 10.0 } else { 0.0 };
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

/// Regenerate the golden buffers and a manifest recording how they were made.
fn bless() {
    // Resolve <repo>/testdata/golden relative to this crate.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/golden")
        .canonicalize()
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/golden"));
    std::fs::create_dir_all(&dir).expect("create golden dir");

    let mut entries = Vec::new();
    for &name in reference::case_names() {
        let buf = reference::render(name);
        let path = dir.join(format!("{name}.json"));
        let json = serde_json::to_string(&buf).expect("serialize buffer");
        std::fs::write(&path, json).expect("write golden");
        entries.push(serde_json::json!({
            "name": name,
            "len": buf.len(),
            "rms": rms(&buf),
        }));
        println!("blessed {name} ({} samples)", buf.len());
    }

    let manifest = serde_json::json!({
        "note": "Tolerance-compared golden buffers. Regenerate with `cargo run -p vactrol-harness -- bless`.",
        "sample_rate": reference::SAMPLE_RATE,
        "case_len": reference::CASE_LEN,
        "ref_seed": reference::REF_SEED,
        "arch": std::env::consts::ARCH,
        "os": std::env::consts::OS,
        "crate_version": env!("CARGO_PKG_VERSION"),
        "goldens": entries,
    });
    let manifest_path = dir.join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .expect("write manifest");
    println!("wrote {}", manifest_path.display());
}

fn rms(buf: &[f32]) -> f32 {
    let s: f64 = buf.iter().map(|x| (*x as f64) * (*x as f64)).sum();
    (s / buf.len() as f64).sqrt() as f32
}
