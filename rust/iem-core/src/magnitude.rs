//! Frequency-response magnitude of a single biquad, evaluated directly on the
//! unit circle. Ported from `EQ_Module.getBiquadMagnitude` in index.html (the
//! curve-plotting / AutoEQ modelling path).
//!
//! IMPORTANT parity note: the original JS high-shelf branch computes
//! `a1 = 2*((A-1) + (A+1)*cosW0)`, whereas the RBJ cookbook (and the audio-path
//! `biquad::design`) use `a1 = 2*((A-1) - (A+1)*cosW0)`. We reproduce the JS
//! exactly in `get_biquad_magnitude` so plots are pixel-identical to the
//! original, and expose `get_biquad_magnitude_rbj` with the corrected sign.

use crate::biquad::FilterType;

#[inline]
fn eval(ftype: FilterType, f: f64, f0: f64, q: f64, g: f64, fs: f64, highshelf_rbj: bool) -> f64 {
    // G == 0 shortcut for the gain filters (matches JS).
    if g == 0.0
        && matches!(
            ftype,
            FilterType::Peaking | FilterType::LowShelf | FilterType::HighShelf
        )
    {
        return 1.0;
    }

    let f_clamped = f.max(1.0);
    let w = 2.0 * std::f64::consts::PI * f_clamped / fs;
    let cos_w = w.cos();
    let sin_w = w.sin();

    let w0 = 2.0 * std::f64::consts::PI * f0 / fs;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let a = (10.0_f64).powf(g / 40.0);

    let (mut b0, mut b1, mut b2, mut a0, mut a1, mut a2) = (0.0, 0.0, 0.0, 1.0, 0.0, 0.0);

    match ftype {
        FilterType::Peaking => {
            let alpha = sin_w0 / (2.0 * q);
            b0 = 1.0 + alpha * a;
            b1 = -2.0 * cos_w0;
            b2 = 1.0 - alpha * a;
            a0 = 1.0 + alpha / a;
            a1 = -2.0 * cos_w0;
            a2 = 1.0 - alpha / a;
        }
        FilterType::LowShelf => {
            let inner = (a + 1.0 / a) * (1.0 / q - 1.0) + 2.0;
            let alpha = (sin_w0 / 2.0) * inner.max(0.0).sqrt();
            let sqrt_a = a.sqrt();
            b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha);
            b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
            b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha);
            a0 = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
            a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
            a2 = (a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha;
        }
        FilterType::HighShelf => {
            let inner = (a + 1.0 / a) * (1.0 / q - 1.0) + 2.0;
            let alpha = (sin_w0 / 2.0) * inner.max(0.0).sqrt();
            let sqrt_a = a.sqrt();
            b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha);
            b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
            b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha);
            a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
            a1 = if highshelf_rbj {
                2.0 * ((a - 1.0) - (a + 1.0) * cos_w0) // corrected RBJ form
            } else {
                2.0 * ((a - 1.0) + (a + 1.0) * cos_w0) // faithful to original JS
            };
            a2 = (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha;
        }
        FilterType::LowPass => {
            let alpha = sin_w0 / (2.0 * q);
            b0 = (1.0 - cos_w0) / 2.0;
            b1 = 1.0 - cos_w0;
            b2 = (1.0 - cos_w0) / 2.0;
            a0 = 1.0 + alpha;
            a1 = -2.0 * cos_w0;
            a2 = 1.0 - alpha;
        }
        FilterType::HighPass => {
            let alpha = sin_w0 / (2.0 * q);
            b0 = (1.0 + cos_w0) / 2.0;
            b1 = -(1.0 + cos_w0);
            b2 = (1.0 + cos_w0) / 2.0;
            a0 = 1.0 + alpha;
            a1 = -2.0 * cos_w0;
            a2 = 1.0 - alpha;
        }
        FilterType::Notch => {
            let alpha = sin_w0 / (2.0 * q);
            b0 = 1.0;
            b1 = -2.0 * cos_w0;
            b2 = 1.0;
            a0 = 1.0 + alpha;
            a1 = -2.0 * cos_w0;
            a2 = 1.0 - alpha;
        }
    }

    let n_b0 = b0 / a0;
    let n_b1 = b1 / a0;
    let n_b2 = b2 / a0;
    let n_a1 = a1 / a0;
    let n_a2 = a2 / a0;

    let cos2w = cos_w * cos_w - sin_w * sin_w;
    let sin2w = 2.0 * sin_w * cos_w;

    let num_real = n_b0 + n_b1 * cos_w + n_b2 * cos2w;
    let num_imag = -(n_b1 * sin_w + n_b2 * sin2w);
    let num_mag2 = num_real * num_real + num_imag * num_imag;

    let den_real = 1.0 + n_a1 * cos_w + n_a2 * cos2w;
    let den_imag = -(n_a1 * sin_w + n_a2 * sin2w);
    let den_mag2 = den_real * den_real + den_imag * den_imag;

    (num_mag2 / den_mag2.max(1e-12)).sqrt()
}

/// Faithful port of `getBiquadMagnitude` (linear magnitude).
#[inline]
pub fn get_biquad_magnitude(ftype: FilterType, f: f64, f0: f64, q: f64, g: f64, fs: f64) -> f64 {
    eval(ftype, f, f0, q, g, fs, false)
}

/// Same, but with the corrected RBJ high-shelf sign.
#[inline]
pub fn get_biquad_magnitude_rbj(
    ftype: FilterType,
    f: f64,
    f0: f64,
    q: f64,
    g: f64,
    fs: f64,
) -> f64 {
    eval(ftype, f, f0, q, g, fs, true)
}
