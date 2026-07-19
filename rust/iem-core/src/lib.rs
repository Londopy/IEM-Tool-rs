//! iem-core — the computational core of IEM-Tool-rs, ported to Rust from IEM Tool.
//!
//! Builds two ways from one source:
//!   * `cdylib` -> `wasm32-unknown-unknown` for the real-time AudioWorklet and
//!     in-page analysis, driven through the C-ABI below.
//!   * `rlib`   -> linked natively into the Tauri backend (see `iem_core::*`),
//!     where the same functions are exposed as Tauri commands.
//!
//! The ABI is deliberately minimal: an exported bump of the standard allocator
//! (`alloc`/`dealloc`) plus plain `extern "C"` functions that read and write
//! f64/f32 arrays through pointers into the module's linear memory.

// The exported C-ABI functions below intentionally take raw pointers into wasm
// linear memory and are the FFI boundary; clippy's suggestion to mark them
// `unsafe` doesn't apply to `extern "C"` exports called from JS.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub mod autoeq;
pub mod biquad;
pub mod curves;
pub mod engine;
pub mod magnitude;

use biquad::FilterType;
use curves::AlignMode;

// ---------------------------------------------------------------------------
// Allocator ABI (so JS can hand us scratch buffers in wasm linear memory)
// ---------------------------------------------------------------------------
use std::alloc::{alloc as sys_alloc, dealloc as sys_dealloc, Layout};

#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    unsafe { sys_alloc(Layout::from_size_align_unchecked(size.max(1), 8)) }
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    if ptr.is_null() {
        return;
    }
    unsafe { sys_dealloc(ptr, Layout::from_size_align_unchecked(size.max(1), 8)) }
}

#[inline]
unsafe fn slice_f64<'a>(ptr: *const f64, n: usize) -> &'a [f64] {
    if ptr.is_null() || n == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(ptr, n)
    }
}
#[inline]
unsafe fn slice_f64_mut<'a>(ptr: *mut f64, n: usize) -> &'a mut [f64] {
    if ptr.is_null() || n == 0 {
        &mut []
    } else {
        std::slice::from_raw_parts_mut(ptr, n)
    }
}
#[inline]
unsafe fn pairs<'a>(ptr: *const f64, n_pairs: usize) -> Vec<(f64, f64)> {
    let flat = slice_f64(ptr, n_pairs * 2);
    (0..n_pairs)
        .map(|i| (flat[2 * i], flat[2 * i + 1]))
        .collect()
}
#[inline]
fn align_mode(mode_hz: f64) -> AlignMode {
    if mode_hz.is_nan() {
        AlignMode::Mean
    } else {
        AlignMode::Hz(mode_hz)
    }
}

// ---------------------------------------------------------------------------
// Scalar / vector frequency-response magnitude
// ---------------------------------------------------------------------------

/// Faithful `getBiquadMagnitude` (linear magnitude).
#[no_mangle]
pub extern "C" fn biquad_magnitude(ftype: i32, f: f64, f0: f64, q: f64, g: f64, fs: f64) -> f64 {
    magnitude::get_biquad_magnitude(FilterType::from_i32(ftype), f, f0, q, g, fs)
}

/// Bug-for-bug original plotting magnitude (legacy high-shelf sign).
#[no_mangle]
pub extern "C" fn biquad_magnitude_legacy(
    ftype: i32,
    f: f64,
    f0: f64,
    q: f64,
    g: f64,
    fs: f64,
) -> f64 {
    magnitude::get_biquad_magnitude_legacy(FilterType::from_i32(ftype), f, f0, q, g, fs)
}

/// Combined chain response in dB: for each target frequency, sum of
/// `20*log10(|H_b|)` across all bands. `bands` is a flat array of
/// `[type, f0, q, g]` repeated `n_bands` times (type stored as f64).
#[no_mangle]
pub extern "C" fn chain_magnitude_db(
    bands: *const f64,
    n_bands: usize,
    freqs: *const f64,
    n_freqs: usize,
    out: *mut f64,
    fs: f64,
) {
    let bands = unsafe { slice_f64(bands, n_bands * 4) };
    let freqs = unsafe { slice_f64(freqs, n_freqs) };
    let out = unsafe { slice_f64_mut(out, n_freqs) };
    for j in 0..n_freqs {
        let mut db = 0.0;
        for b in 0..n_bands {
            let ftype = FilterType::from_i32(bands[4 * b] as i32);
            let f0 = bands[4 * b + 1];
            let q = bands[4 * b + 2];
            let g = bands[4 * b + 3];
            let m = magnitude::get_biquad_magnitude(ftype, freqs[j], f0, q, g, fs);
            db += 20.0 * m.max(1e-10).log10();
        }
        out[j] = db;
    }
}

// ---------------------------------------------------------------------------
// Curve utilities
// ---------------------------------------------------------------------------

/// `normalizeTo75dB`. `data`/`out` are (hz,db) pair arrays of length `n`.
/// `mode_hz` NaN => 'mean' alignment; otherwise align to nearest that Hz.
#[no_mangle]
pub extern "C" fn normalize_to_75db(
    data: *const f64,
    n: usize,
    mode_hz: f64,
    target_db: f64,
    out: *mut f64,
) {
    let pts = unsafe { pairs(data, n) };
    let res = curves::normalize_to_75db(&pts, align_mode(mode_hz), target_db);
    let out = unsafe { slice_f64_mut(out, n * 2) };
    for i in 0..n {
        out[2 * i] = res[i].0;
        out[2 * i + 1] = res[i].1;
    }
}

/// `cubicSplineInterpolate`. `points` = (hz,db) pairs (n_points); writes
/// `n_targets` values at `targets` into `out`.
#[no_mangle]
pub extern "C" fn cubic_spline(
    points: *const f64,
    n_points: usize,
    targets: *const f64,
    n_targets: usize,
    out: *mut f64,
) {
    let pts = unsafe { pairs(points, n_points) };
    let tf = unsafe { slice_f64(targets, n_targets) };
    let res = curves::cubic_spline_interpolate(&pts, tf);
    unsafe { slice_f64_mut(out, n_targets) }.copy_from_slice(&res);
}

/// Log-linear interpolation matching `DSP.interpolate` (no role shift).
/// `points` = (hz,db) pairs sorted ascending by hz.
#[no_mangle]
pub extern "C" fn interp_loglinear(
    points: *const f64,
    n_points: usize,
    targets: *const f64,
    n_targets: usize,
    out: *mut f64,
) {
    let curve = unsafe { pairs(points, n_points) };
    let tf = unsafe { slice_f64(targets, n_targets) };
    let out = unsafe { slice_f64_mut(out, n_targets) };
    if curve.is_empty() {
        for v in out.iter_mut() {
            *v = 0.0;
        }
        return;
    }
    let last = curve.len() - 1;
    for (k, &f) in tf.iter().enumerate() {
        if f <= curve[0].0 {
            out[k] = curve[0].1;
            continue;
        }
        if f >= curve[last].0 {
            out[k] = curve[last].1;
            continue;
        }
        let (mut low, mut high) = (0i64, last as i64);
        let mut exact = None;
        while low <= high {
            let mid = (low + high) >> 1;
            let xm = curve[mid as usize].0;
            if xm == f {
                exact = Some(curve[mid as usize].1);
                break;
            }
            if xm < f {
                low = mid + 1;
            } else {
                high = mid - 1;
            }
        }
        if let Some(v) = exact {
            out[k] = v;
            continue;
        }
        let (hi, lo) = (high as usize, low as usize);
        let x0 = curve[hi].0.log10();
        let y0 = curve[hi].1;
        let x1 = curve[lo].0.log10();
        let y1 = curve[lo].1;
        out[k] = y0 + (f.log10() - x0) * (y1 - y0) / (x1 - x0);
    }
}

/// `gaussianSmooth` in place-friendly form.
#[no_mangle]
pub extern "C" fn gaussian_smooth(
    freqs: *const f64,
    values: *const f64,
    n: usize,
    octave_bw: f64,
    out: *mut f64,
) {
    let fr = unsafe { slice_f64(freqs, n) };
    let va = unsafe { slice_f64(values, n) };
    let res = curves::gaussian_smooth(fr, va, octave_bw);
    unsafe { slice_f64_mut(out, n) }.copy_from_slice(&res);
}

/// `generateLogGrid(num_points)` written into `out`.
#[no_mangle]
pub extern "C" fn generate_log_grid(num_points: usize, out: *mut f64) {
    let g = curves::generate_log_grid(num_points);
    unsafe { slice_f64_mut(out, num_points) }.copy_from_slice(&g);
}

// ---------------------------------------------------------------------------
// AutoEQ solver
// ---------------------------------------------------------------------------

/// Coordinate-descent AutoEQ. Fills `out_gains` (n_bands) and returns the
/// optimal pre-amp (dB).
#[no_mangle]
pub extern "C" fn autoeq_solve(
    target_correction: *const f64,
    freqs: *const f64,
    n_points: usize,
    band_freqs: *const f64,
    band_qs: *const f64,
    n_bands: usize,
    fs: f64,
    out_gains: *mut f64,
) -> f64 {
    let tc = unsafe { slice_f64(target_correction, n_points) };
    let fr = unsafe { slice_f64(freqs, n_points) };
    let bf = unsafe { slice_f64(band_freqs, n_bands) };
    let bq = unsafe { slice_f64(band_qs, n_bands) };
    let res = autoeq::solve(tc, fr, bf, bq, fs);
    unsafe { slice_f64_mut(out_gains, n_bands) }.copy_from_slice(&res.gains);
    res.preamp
}

// ---------------------------------------------------------------------------
// Real-time engine (opaque handle)
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn engine_new(sample_rate: f64) -> *mut engine::EqEngine {
    Box::into_raw(Box::new(engine::EqEngine::new(sample_rate)))
}

#[no_mangle]
pub extern "C" fn engine_free(ptr: *mut engine::EqEngine) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

#[no_mangle]
pub extern "C" fn engine_set_sample_rate(ptr: *mut engine::EqEngine, sample_rate: f64) {
    if let Some(e) = unsafe { ptr.as_mut() } {
        e.set_sample_rate(sample_rate);
    }
}

#[no_mangle]
pub extern "C" fn engine_set_preamp(ptr: *mut engine::EqEngine, preamp_db: f64) {
    if let Some(e) = unsafe { ptr.as_mut() } {
        e.set_preamp_db(preamp_db);
    }
}

/// bank: 0 = EQ, 1 = Sim, 2 = Crossover.
#[no_mangle]
pub extern "C" fn engine_update_filter(
    ptr: *mut engine::EqEngine,
    bank: i32,
    index: usize,
    ftype: i32,
    freq: f64,
    gain: f64,
    q: f64,
    bypassed: i32,
) {
    if let Some(e) = unsafe { ptr.as_mut() } {
        let bank = match bank {
            1 => engine::Bank::Sim,
            2 => engine::Bank::Xo,
            _ => engine::Bank::Eq,
        };
        e.update_filter(
            bank,
            index,
            FilterType::from_i32(ftype),
            freq,
            gain,
            q,
            bypassed != 0,
        );
    }
}

#[no_mangle]
pub extern "C" fn engine_set_crossover(
    ptr: *mut engine::EqEngine,
    enabled: i32,
    xo_type: i32,
    g0: f64,
    g1: f64,
    g2: f64,
    g3: f64,
    g4: f64,
) {
    if let Some(e) = unsafe { ptr.as_mut() } {
        e.set_crossover(enabled != 0, xo_type, [g0, g1, g2, g3, g4]);
    }
}

#[no_mangle]
pub extern "C" fn engine_reset(ptr: *mut engine::EqEngine) {
    if let Some(e) = unsafe { ptr.as_mut() } {
        e.reset();
    }
}

/// Process a stereo block of `n` frames. All four buffers are `n`-length f32.
#[no_mangle]
pub extern "C" fn engine_process(
    ptr: *mut engine::EqEngine,
    in_l: *const f32,
    in_r: *const f32,
    out_l: *mut f32,
    out_r: *mut f32,
    n: usize,
) {
    let e = match unsafe { ptr.as_mut() } {
        Some(e) => e,
        None => return,
    };
    unsafe {
        let il = std::slice::from_raw_parts(in_l, n);
        let ir = std::slice::from_raw_parts(in_r, n);
        let ol = std::slice::from_raw_parts_mut(out_l, n);
        let or = std::slice::from_raw_parts_mut(out_r, n);
        e.process(il, ir, ol, or, n);
    }
}
