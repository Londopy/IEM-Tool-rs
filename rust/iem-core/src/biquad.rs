//! Biquad filter: RBJ Audio-EQ-Cookbook coefficient design plus a stereo,
//! parameter-interpolating time-domain filter. Ported faithfully from the
//! original `dsp-processor.js` (the real-time AudioWorklet path).

/// Filter kinds, matching the string types used in the original JS.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilterType {
    Peaking = 0,
    LowShelf = 1,
    HighShelf = 2,
    LowPass = 3,
    HighPass = 4,
    Notch = 5,
}

impl FilterType {
    #[inline]
    pub fn from_i32(v: i32) -> FilterType {
        match v {
            1 => FilterType::LowShelf,
            2 => FilterType::HighShelf,
            3 => FilterType::LowPass,
            4 => FilterType::HighPass,
            5 => FilterType::Notch,
            _ => FilterType::Peaking,
        }
    }
}

/// Normalized biquad coefficients (a0 divided out).
#[derive(Clone, Copy)]
pub struct Coeffs {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

/// Design normalized biquad coefficients from (type, freq, gain_db, Q, sample_rate).
///
/// This mirrors `BiquadFilter.calculateCoefficients` in dsp-processor.js — the
/// RBJ cookbook forms actually used for audio. (Note: the *plotting* function
/// `get_biquad_magnitude` in `magnitude.rs` reproduces a separate JS routine
/// that has a high-shelf `a1` sign quirk; this one is the correct RBJ form.)
pub fn design(ftype: FilterType, freq: f64, gain_db: f64, q: f64, sample_rate: f64) -> Coeffs {
    let w0 = 2.0 * std::f64::consts::PI * freq / sample_rate;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q);
    let a = (10.0_f64).powf(gain_db / 40.0);

    let (mut b0, mut b1, mut b2, mut a0, mut a1, mut a2) = (1.0, 0.0, 0.0, 1.0, 0.0, 0.0);

    match ftype {
        FilterType::Peaking => {
            b0 = 1.0 + alpha * a;
            b1 = -2.0 * cos_w0;
            b2 = 1.0 - alpha * a;
            a0 = 1.0 + alpha / a;
            a1 = -2.0 * cos_w0;
            a2 = 1.0 - alpha / a;
        }
        FilterType::LowShelf => {
            let inner = (a + 1.0 / a) * (1.0 / q - 1.0) + 2.0;
            let alpha_s = (sin_w0 / 2.0) * inner.max(0.0).sqrt();
            let sqrt_a = a.sqrt();
            b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha_s);
            b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
            b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha_s);
            a0 = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha_s;
            a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
            a2 = (a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha_s;
        }
        FilterType::HighShelf => {
            let inner = (a + 1.0 / a) * (1.0 / q - 1.0) + 2.0;
            let alpha_s = (sin_w0 / 2.0) * inner.max(0.0).sqrt();
            let sqrt_a = a.sqrt();
            b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha_s);
            b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
            b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha_s);
            a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha_s;
            a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
            a2 = (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha_s;
        }
        FilterType::LowPass => {
            b0 = (1.0 - cos_w0) / 2.0;
            b1 = 1.0 - cos_w0;
            b2 = (1.0 - cos_w0) / 2.0;
            a0 = 1.0 + alpha;
            a1 = -2.0 * cos_w0;
            a2 = 1.0 - alpha;
        }
        FilterType::HighPass => {
            b0 = (1.0 + cos_w0) / 2.0;
            b1 = -(1.0 + cos_w0);
            b2 = (1.0 + cos_w0) / 2.0;
            a0 = 1.0 + alpha;
            a1 = -2.0 * cos_w0;
            a2 = 1.0 - alpha;
        }
        FilterType::Notch => {
            b0 = 1.0;
            b1 = -2.0 * cos_w0;
            b2 = 1.0;
            a0 = 1.0 + alpha;
            a1 = -2.0 * cos_w0;
            a2 = 1.0 - alpha;
        }
    }

    let div = if a0.is_finite() && a0 != 0.0 { a0 } else { 1.0 };
    Coeffs {
        b0: b0 / div,
        b1: b1 / div,
        b2: b2 / div,
        a1: a1 / div,
        a2: a2 / div,
    }
}

/// Stereo, parameter-interpolating biquad — a direct port of the JS
/// `BiquadFilter` class, including the RECALC_INTERVAL coefficient throttling,
/// denormal flushing, and parameter-space smoothing.
pub struct Biquad {
    // Active coefficients
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    // Active parameters
    frequency: f64,
    gain: f64,
    q: f64,
    // Target parameters
    target_frequency: f64,
    target_gain: f64,
    target_q: f64,
    // Stereo state
    s1_l: f64,
    s2_l: f64,
    s1_r: f64,
    s2_r: f64,
    pub bypassed: bool,
    ftype: FilterType,
    recalc_interval: i32,
    recalc_counter: i32,
    coeffs_current: bool,
}

impl Biquad {
    pub fn new() -> Biquad {
        Biquad {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            frequency: 1000.0,
            gain: 0.0,
            q: 1.0,
            target_frequency: 1000.0,
            target_gain: 0.0,
            target_q: 1.0,
            s1_l: 0.0,
            s2_l: 0.0,
            s1_r: 0.0,
            s2_r: 0.0,
            bypassed: true,
            ftype: FilterType::Peaking,
            recalc_interval: 8,
            recalc_counter: 1,
            coeffs_current: true,
        }
    }

    pub fn reset(&mut self) {
        self.s1_l = 0.0;
        self.s2_l = 0.0;
        self.s1_r = 0.0;
        self.s2_r = 0.0;
    }

    fn calculate(&mut self, sample_rate: f64) {
        let c = design(self.ftype, self.frequency, self.gain, self.q, sample_rate);
        self.b0 = c.b0;
        self.b1 = c.b1;
        self.b2 = c.b2;
        self.a1 = c.a1;
        self.a2 = c.a2;
    }

    /// Set targets. `was_bypassed` is the PREVIOUS bypass state (caller updates
    /// `self.bypassed` before calling), matching the JS contract.
    pub fn update_coefficients(
        &mut self,
        ftype: FilterType,
        freq: f64,
        gain: f64,
        q: f64,
        sample_rate: f64,
        was_bypassed: bool,
    ) {
        self.ftype = ftype;
        let f = if freq.is_finite() { freq } else { 1000.0 };
        let g = if gain.is_finite() { gain } else { 0.0 };
        let qq = if q.is_finite() { q } else { 1.0 };
        self.target_frequency = f.clamp(10.0, 22000.0);
        self.target_gain = g.clamp(-40.0, 40.0);
        self.target_q = qq.clamp(0.01, 50.0);

        if was_bypassed {
            self.frequency = self.target_frequency;
            self.gain = self.target_gain;
            self.q = self.target_q;
            self.calculate(sample_rate);
            self.coeffs_current = true;
            self.recalc_counter = self.recalc_interval;
        } else {
            self.coeffs_current = false;
            self.recalc_counter = 1;
        }
    }

    #[inline]
    pub fn process_sample_l(&mut self, x: f64, smoothing_factor: f64, sample_rate: f64) -> f64 {
        let freq_diff = (self.target_frequency - self.frequency).abs();
        let gain_diff = (self.target_gain - self.gain).abs();
        let q_diff = (self.target_q - self.q).abs();
        let settled = freq_diff <= 0.01 && gain_diff <= 0.01 && q_diff <= 0.001;

        if !settled {
            self.frequency += (self.target_frequency - self.frequency) * smoothing_factor;
            self.gain += (self.target_gain - self.gain) * smoothing_factor;
            self.q += (self.target_q - self.q) * smoothing_factor;
            self.recalc_counter -= 1;
            if self.recalc_counter <= 0 {
                self.calculate(sample_rate);
                self.recalc_counter = self.recalc_interval;
            }
            self.coeffs_current = false;
        } else if !self.coeffs_current {
            self.frequency = self.target_frequency;
            self.gain = self.target_gain;
            self.q = self.target_q;
            self.calculate(sample_rate);
            self.coeffs_current = true;
        }

        if !self.s1_l.is_finite() || !self.s2_l.is_finite() {
            self.s1_l = 0.0;
            self.s2_l = 0.0;
        }

        let y = x * self.b0 + self.s1_l;
        self.s1_l = x * self.b1 - self.a1 * y + self.s2_l;
        self.s2_l = x * self.b2 - self.a2 * y;

        if self.s1_l.abs() < 1e-15 {
            self.s1_l = 0.0;
        }
        if self.s2_l.abs() < 1e-15 {
            self.s2_l = 0.0;
        }
        y
    }

    /// Process a whole block for both channels.
    ///
    /// L and R are interleaved per sample (L then R for sample i) which is
    /// exactly the order the original per-sample chain used, so results are
    /// bit-identical - but the per-filter bypass check and call overhead now
    /// happen once per block instead of once per sample.
    #[inline]
    pub fn process_block(
        &mut self,
        l: &mut [f64],
        r: &mut [f64],
        smoothing_factor: f64,
        sample_rate: f64,
    ) {
        debug_assert_eq!(l.len(), r.len());
        for i in 0..l.len() {
            l[i] = self.process_sample_l(l[i], smoothing_factor, sample_rate);
            r[i] = self.process_sample_r(r[i]);
        }
    }

    #[inline]
    pub fn process_sample_r(&mut self, x: f64) -> f64 {
        if !self.s1_r.is_finite() || !self.s2_r.is_finite() {
            self.s1_r = 0.0;
            self.s2_r = 0.0;
        }
        let y = x * self.b0 + self.s1_r;
        self.s1_r = x * self.b1 - self.a1 * y + self.s2_r;
        self.s2_r = x * self.b2 - self.a2 * y;
        if self.s1_r.abs() < 1e-15 {
            self.s1_r = 0.0;
        }
        if self.s2_r.abs() < 1e-15 {
            self.s2_r = 0.0;
        }
        y
    }
}
