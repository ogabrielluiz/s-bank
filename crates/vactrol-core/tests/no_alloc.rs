//! Phase 1 acceptance: `process_block` performs no heap allocation on the audio
//! thread. `assert_no_alloc` installs an allocator shim that flags any alloc/free
//! inside the guarded closure (debug builds; `cargo test` is a debug build).

use assert_no_alloc::*;
use vactrol_core::{Lpg, Mode, Params};

#[cfg(debug_assertions)]
#[global_allocator]
static A: AllocDisabler = AllocDisabler;

#[test]
fn process_block_does_not_allocate() {
    let mut lpg = Lpg::new(48_000.0);
    lpg.set_params(Params {
        mode: Mode::Both,
        resonance: 0.5,
        cv_offset: 0.0,
        drive: 1.0,
        oversample: 2,
    });

    // Allocate the buffers up front, outside the guarded region.
    let audio: Vec<f32> = (0..256).map(|i| (i as f32 * 0.1).sin()).collect();
    let cv = vec![5.0f32; 256];
    let mut out = vec![0.0f32; 256];

    assert_no_alloc(|| {
        lpg.process_block(&audio, &cv, &mut out);
    });

    assert!(out.iter().all(|x| x.is_finite()));
}
