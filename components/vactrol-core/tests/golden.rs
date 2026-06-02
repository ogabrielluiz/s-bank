//! Phase 4 acceptance: tolerance-based golden-reference regression.
//!
//! Compares fresh renders of each reference case against the committed golden
//! buffers under `testdata/golden/`. Never bit-exact: float DSP output varies
//! across compiler/opt/SIMD/fast-math/arch, so we assert a small per-sample
//! max-abs-error AND a high normalized cross-correlation. Regenerate goldens
//! deliberately with `cargo run -p vactrol-harness -- bless`.

use std::path::PathBuf;

use vactrol_core::reference;

/// Absolute per-sample tolerance. Generous vs the reference platform's own output
/// (which is ~bit-identical) but tight enough to catch real regressions.
const MAX_ABS_ERR: f32 = 1.0e-3;
/// Minimum normalized cross-correlation.
const MIN_CORR: f32 = 0.9999;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/golden")
}

fn load_golden(name: &str) -> Vec<f32> {
    let path = golden_dir().join(format!("{name}.json"));
    let json = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden '{}' ({e}). Generate with `cargo run -p vactrol-harness -- bless`",
            path.display()
        )
    });
    serde_json::from_str(&json).expect("parse golden")
}

fn max_abs_err(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max)
}

fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| *x as f64 * *y as f64).sum();
    let na: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 1.0;
    }
    (dot / (na * nb)) as f32
}

#[test]
fn renders_match_committed_goldens() {
    for &name in reference::case_names() {
        let fresh = reference::render(name);
        let golden = load_golden(name);
        assert_eq!(fresh.len(), golden.len(), "length mismatch for '{name}'");

        let err = max_abs_err(&fresh, &golden);
        let corr = correlation(&fresh, &golden);
        assert!(
            err < MAX_ABS_ERR,
            "'{name}' max-abs-error {err:.2e} exceeds {MAX_ABS_ERR:.0e} (regression or stale golden)"
        );
        assert!(
            corr > MIN_CORR,
            "'{name}' correlation {corr:.6} below {MIN_CORR} (regression or stale golden)"
        );
    }
}
