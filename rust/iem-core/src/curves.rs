//! Curve utilities ported from the `CurveUtils` object in index.html:
//! log-frequency grid, 75 dB normalization, natural cubic-spline interpolation
//! in log-frequency, Gaussian (fractional-octave) smoothing, and trimmed-mean
//! curve averaging.

/// Reference alignment mode for normalization.
#[derive(Clone, Copy)]
pub enum AlignMode {
    /// Mean of all points with 500 <= hz <= 2000.
    Mean,
    /// Value at the point whose frequency is nearest `hz`.
    Hz(f64),
}

/// `CurveUtils.generateLogGrid` — `num_points` log-spaced freqs from 20..20000.
pub fn generate_log_grid(num_points: usize) -> Vec<f64> {
    let (min_f, max_f) = (20.0_f64, 20000.0_f64);
    let mut grid = Vec::with_capacity(num_points);
    for i in 0..num_points {
        let t = if num_points > 1 {
            i as f64 / (num_points - 1) as f64
        } else {
            0.0
        };
        grid.push(min_f * (max_f / min_f).powf(t));
    }
    grid
}

/// `CurveUtils.normalizeTo75dB` — shift a curve so the reference point reads
/// `target_db`. `data` is (hz, db) pairs; returns shifted (hz, db) pairs.
pub fn normalize_to_75db(data: &[(f64, f64)], mode: AlignMode, target_db: f64) -> Vec<(f64, f64)> {
    if data.is_empty() {
        return Vec::new();
    }
    let ref_db = match mode {
        AlignMode::Mean => {
            let mut sum = 0.0;
            let mut count = 0usize;
            for &(hz, db) in data {
                if hz >= 500.0 && hz <= 2000.0 {
                    sum += db;
                    count += 1;
                }
            }
            if count > 0 {
                sum / count as f64
            } else {
                data[0].1
            }
        }
        AlignMode::Hz(freq) => {
            let freq = if freq.is_finite() { freq } else { 500.0 };
            let mut min_diff = f64::INFINITY;
            let mut r = 0.0;
            for &(hz, db) in data {
                let diff = (hz - freq).abs();
                if diff < min_diff {
                    min_diff = diff;
                    r = db;
                }
            }
            r
        }
    };
    data.iter()
        .map(|&(hz, db)| (hz, db - ref_db + target_db))
        .collect()
}

/// `CurveUtils.cubicSplineInterpolate` — natural cubic spline through
/// (log10(hz), db) evaluated at `target_freqs`. Flat extrapolation past ends.
pub fn cubic_spline_interpolate(points: &[(f64, f64)], target_freqs: &[f64]) -> Vec<f64> {
    let n = points.len();
    if n < 2 {
        return vec![75.0; target_freqs.len()];
    }
    let mut x = vec![0.0; n];
    let mut a = vec![0.0; n];
    for i in 0..n {
        x[i] = points[i].0.log10();
        a[i] = points[i].1;
    }

    let mut h = vec![0.0; n - 1];
    for i in 0..n - 1 {
        h[i] = x[i + 1] - x[i];
    }

    let mut alpha = vec![0.0; n];
    for i in 1..n - 1 {
        alpha[i] = (3.0 / h[i]) * (a[i + 1] - a[i]) - (3.0 / h[i - 1]) * (a[i] - a[i - 1]);
    }

    let mut l = vec![0.0; n];
    let mut mu = vec![0.0; n];
    let mut z = vec![0.0; n];
    l[0] = 1.0;
    for i in 1..n - 1 {
        l[i] = 2.0 * (x[i + 1] - x[i - 1]) - h[i - 1] * mu[i - 1];
        mu[i] = h[i] / l[i];
        z[i] = (alpha[i] - h[i - 1] * z[i - 1]) / l[i];
    }
    l[n - 1] = 1.0;
    z[n - 1] = 0.0;

    let mut c = vec![0.0; n];
    let mut b = vec![0.0; n - 1];
    let mut d = vec![0.0; n - 1];
    for j in (0..n - 1).rev() {
        c[j] = z[j] - mu[j] * c[j + 1];
        b[j] = (a[j + 1] - a[j]) / h[j] - h[j] * (c[j + 1] + 2.0 * c[j]) / 3.0;
        d[j] = (c[j + 1] - c[j]) / (3.0 * h[j]);
    }

    let mut out = Vec::with_capacity(target_freqs.len());
    for &tf in target_freqs {
        let val = tf.log10();
        if val <= x[0] {
            out.push(a[0]);
            continue;
        }
        if val >= x[n - 1] {
            out.push(a[n - 1]);
            continue;
        }
        // Binary search for the interval, matching the JS (idx = max(0, high)).
        // Interval index: the largest `idx` with x[idx] <= val, in [0, n-2].
        // (Equivalent to the JS binary search for the grid targets the app uses,
        // but also correct when `val` exactly equals an interior knot — where the
        // original JS read past b/c/d and produced NaN.)
        let idx = x
            .partition_point(|&xi| xi <= val)
            .saturating_sub(1)
            .min(n - 2);
        let dx = val - x[idx];
        out.push(a[idx] + b[idx] * dx + c[idx] * dx * dx + d[idx] * dx * dx * dx);
    }
    out
}

/// `CurveUtils.gaussianSmooth` — Gaussian smoothing in log-frequency with a
/// bandwidth expressed in octaves. Truncated at 3 sigma like the JS.
pub fn gaussian_smooth(freqs: &[f64], values: &[f64], octave_bandwidth: f64) -> Vec<f64> {
    let n = freqs.len();
    let mut smoothed = vec![0.0; n];
    let log_freqs: Vec<f64> = freqs.iter().map(|f| f.log10()).collect();
    let sigma = octave_bandwidth * (2.0_f64).log10();
    let factor = -1.0 / (2.0 * sigma * sigma);
    let cutoff = 9.0 * sigma * sigma;

    for i in 0..n {
        let mut weight_sum = 0.0;
        let mut value_sum = 0.0;
        for j in 0..n {
            let diff = log_freqs[i] - log_freqs[j];
            let dist_sq = diff * diff;
            if dist_sq > cutoff {
                continue;
            }
            let w = (dist_sq * factor).exp();
            value_sum += values[j] * w;
            weight_sum += w;
        }
        smoothed[i] = if weight_sum > 0.0 {
            value_sum / weight_sum
        } else {
            values[i]
        };
    }
    smoothed
}

/// `CurveUtils.averageCurves` — normalize + spline each raw curve onto `log_grid`,
/// take a 15%-trimmed mean across curves (when >= 4), then Gaussian-smooth at
/// 0.05 octave. Each curve is (hz, db) pairs.
pub fn average_curves(
    curves: &[Vec<(f64, f64)>],
    log_grid: &[f64],
    mode: AlignMode,
    target_db: f64,
) -> Vec<f64> {
    let len = log_grid.len();
    if curves.is_empty() {
        return vec![75.0; len];
    }
    let matrix: Vec<Vec<f64>> = curves
        .iter()
        .map(|c| {
            let norm = normalize_to_75db(c, mode, target_db);
            cubic_spline_interpolate(&norm, log_grid)
        })
        .collect();

    if curves.len() == 1 {
        return matrix.into_iter().next().unwrap();
    }

    let num_curves = curves.len();
    let mut averaged = vec![0.0; len];
    for i in 0..len {
        let mut values_at_freq: Vec<f64> = (0..num_curves).map(|j| matrix[j][i]).collect();
        values_at_freq.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let (mut sum, mut count) = (0.0, 0usize);
        if num_curves >= 4 {
            let start = (num_curves as f64 * 0.15).floor() as usize;
            let end = num_curves - start;
            for k in start..end {
                sum += values_at_freq[k];
                count += 1;
            }
        } else {
            for k in 0..num_curves {
                sum += values_at_freq[k];
                count += 1;
            }
        }
        averaged[i] = sum / count as f64;
    }
    gaussian_smooth(log_grid, &averaged, 0.05)
}
