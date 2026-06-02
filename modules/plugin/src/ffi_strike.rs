//! C ABI for the **Strike** instrument, re-exported from this (the single VCV plugin)
//! staticlib so both instruments share one Rust runtime. `StrikeVoice` is an opaque
//! pointer on the C side. Every entry point is panic-safe; only create/destroy touch
//! the heap.

use std::panic::{catch_unwind, AssertUnwindSafe};

use strike_core::{ImperfectionConfig, Strike, StrikeParams};

/// Opaque handle to one Strike voice (wraps the `strike-core` engine).
pub struct StrikeVoice(Strike);

/// Create a Strike voice. Returns null on failure.
///
/// # Safety
/// Release with [`strike_destroy`].
#[no_mangle]
pub extern "C" fn strike_create(sample_rate: f32) -> *mut StrikeVoice {
    catch_unwind(|| Box::into_raw(Box::new(StrikeVoice(Strike::new(sample_rate)))))
        .unwrap_or(std::ptr::null_mut())
}

/// Destroy a voice from [`strike_create`].
///
/// # Safety
/// `ptr` must come from `strike_create` (or be null), used at most once.
#[no_mangle]
pub unsafe extern "C" fn strike_destroy(ptr: *mut StrikeVoice) {
    if ptr.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| drop(Box::from_raw(ptr))));
}

/// # Safety
/// `ptr` must be a valid voice or null.
#[no_mangle]
pub unsafe extern "C" fn strike_set_sample_rate(ptr: *mut StrikeVoice, sample_rate: f32) {
    if ptr.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| (*ptr).0.set_sample_rate(sample_rate)));
}

/// Set knob params. `open`, `decay`, `material` are all `0..=1`.
///
/// # Safety
/// `ptr` must be a valid voice or null.
#[no_mangle]
pub unsafe extern "C" fn strike_set_params(
    ptr: *mut StrikeVoice,
    open: f32,
    decay: f32,
    material: f32,
) {
    if ptr.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        (*ptr).0.set_params(StrikeParams {
            open,
            decay,
            material,
        });
    }));
}

/// Configure the optional imperfection layer. `enabled`: 0 = off, nonzero = on.
///
/// # Safety
/// `ptr` must be a valid voice or null.
#[no_mangle]
pub unsafe extern "C" fn strike_set_imperfection(
    ptr: *mut StrikeVoice,
    enabled: u32,
    noise_amp: f32,
    drift_amount: f32,
) {
    if ptr.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        (*ptr).0.set_imperfection(ImperfectionConfig {
            enabled: enabled != 0,
            noise_amp,
            drift_amount,
        });
    }));
}

/// Process one sample. `ctrl01` is the pre-EG opening in `0..1`; `decay_mod` is the
/// additive DECAY CV in `0..1` units; `hit_v` is the HIT trigger in volts. Feed a DC
/// value into `audio_in` (with no patched input) for a clean ping / envelope-out.
/// Returns 0.0 on a null pointer or panic.
///
/// # Safety
/// `ptr` must be a valid voice or null.
#[no_mangle]
pub unsafe extern "C" fn strike_process_sample(
    ptr: *mut StrikeVoice,
    audio_in: f32,
    ctrl01: f32,
    decay_mod: f32,
    hit_v: f32,
) -> f32 {
    if ptr.is_null() {
        return 0.0;
    }
    catch_unwind(AssertUnwindSafe(|| {
        (*ptr).0.process_sample(audio_in, ctrl01, decay_mod, hit_v)
    }))
    .unwrap_or(0.0)
}

/// Last gate-opening value `0..=1` (for an "openness" LED). 0.0 on null/panic.
///
/// # Safety
/// `ptr` must be a valid voice or null.
#[no_mangle]
pub unsafe extern "C" fn strike_last_control(ptr: *mut StrikeVoice) -> f32 {
    if ptr.is_null() {
        return 0.0;
    }
    catch_unwind(AssertUnwindSafe(|| (*ptr).0.last_control())).unwrap_or(0.0)
}
