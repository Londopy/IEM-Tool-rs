//! AutoEQ solver ported from `PEQDB_Module.generateLeastSquaresAutoEQ`.
//! A weighted, coordinate-descent least-squares fit of peaking-filter gains to
//! a target correction curve (target minus base, in dB), with a perceptual
//! frequency weighting and a computed optimal pre-amp.

use crate::biquad::FilterType;
use crate::magnitude::get_biquad_magnitude;

pub struct AutoEqResult {
    pub gains: Vec<f64>,
    pub preamp: f64,
}

/// Perceptual weight per the original JS.
#[inline]
fn perceptual_weight(f: f64) -> f64 {
    if f < 40.0 {
        0.3
    } else if f < 100.0 {
        0.8
    } else if f < 3000.0 {
        1.5
    } else if f < 8000.0 {
        1.0
    } else {
        0.2
    }
}

/// Solve for per-band gains (dB) fitting `target_correction` (per-frequency dB)
/// with peaking filters at (`band_freqs`, `band_qs`). `fs` is the sample rate.
///
/// Mirrors the original: 20 coordinate-descent sweeps, gains clamped to +/-12 dB,
/// unit band responses precomputed in dB, and a returned optimal pre-amp of
/// `-max(model dB)` (or 0 if the model never rises above 0 dB).
pub fn solve(
    target_correction: &[f64],
    freqs: &[f64],
    band_freqs: &[f64],
    band_qs: &[f64],
    fs: f64,
) -> AutoEqResult {
    let points = freqs.len();
    let n_bands = band_freqs.len();

    // Precompute unit (1 dB gain) magnitude responses in dB and the weights.
    let mut band_responses = vec![vec![0.0f64; points]; n_bands];
    for b in 0..n_bands {
        for j in 0..points {
            let m = get_biquad_magnitude(
                FilterType::Peaking,
                freqs[j],
                band_freqs[b],
                band_qs[b],
                1.0,
                fs,
            );
            band_responses[b][j] = 20.0 * m.max(1e-10).log10();
        }
    }
    let weights: Vec<f64> = freqs.iter().map(|&f| perceptual_weight(f)).collect();

    let mut gains = vec![0.0f64; n_bands];

    let iterations = 20;
    for _ in 0..iterations {
        for b in 0..n_bands {
            let mut num = 0.0;
            let mut den = 0.0;
            for j in 0..points {
                let mut modeled = 0.0;
                for k in 0..n_bands {
                    if k != b {
                        modeled += band_responses[k][j] * gains[k];
                    }
                }
                let residual = target_correction[j] - modeled;
                num += residual * band_responses[b][j] * weights[j];
                den += band_responses[b][j] * band_responses[b][j] * weights[j];
            }
            if den > 1e-6 {
                let ideal = num / den;
                gains[b] = ideal.clamp(-12.0, 12.0);
            }
        }
    }

    // Optimal pre-amp from the peak of the combined response.
    let mut max_model_db = 0.0;
    for j in 0..points {
        let mut model_db = 0.0;
        for b in 0..n_bands {
            model_db += band_responses[b][j] * gains[b];
        }
        if model_db > max_model_db {
            max_model_db = model_db;
        }
    }
    let preamp = if max_model_db > 0.0 {
        -max_model_db
    } else {
        0.0
    };

    AutoEqResult { gains, preamp }
}
