//! Real-time stereo EQ engine — a faithful port of `DspProcessor` from
//! dsp-processor.js: interpolated pre-amp, an 80-band parametric chain, a
//! 15-band acoustics/simulation chain, and a 3/4/5-way active crossover.
//!
//! Designed to be driven from a WebAudio `AudioWorkletProcessor` compiled to
//! WASM (process whole 128-sample blocks per call), or natively.

use crate::biquad::{Biquad, FilterType};

const SMOOTHING_TIME_CONSTANT_SECONDS: f64 = 200.0 / 44100.0;

fn compute_smoothing_factor(sample_rate: f64) -> f64 {
    1.0 - (-1.0 / (SMOOTHING_TIME_CONSTANT_SECONDS * sample_rate)).exp()
}

pub const NUM_EQ: usize = 80;
pub const NUM_SIM: usize = 15;
pub const NUM_XO: usize = 10;

/// Which filter bank an update targets.
pub enum Bank {
    Eq,
    Sim,
    Xo,
}

pub struct EqEngine {
    sample_rate: f64,
    smoothing_factor: f64,
    preamp_gain: f64,
    target_preamp_gain: f64,
    filters: Vec<Biquad>,
    sim_filters: Vec<Biquad>,
    xo_enabled: bool,
    xo_type: i32, // 3, 4, or 5
    xo_gains: [f64; 5],
    xo_filters: Vec<Biquad>,
}

impl EqEngine {
    pub fn new(sample_rate: f64) -> EqEngine {
        EqEngine {
            sample_rate,
            smoothing_factor: compute_smoothing_factor(sample_rate),
            preamp_gain: 1.0,
            target_preamp_gain: 1.0,
            filters: (0..NUM_EQ).map(|_| Biquad::new()).collect(),
            sim_filters: (0..NUM_SIM).map(|_| Biquad::new()).collect(),
            xo_enabled: false,
            xo_type: 3,
            xo_gains: [1.0; 5],
            xo_filters: (0..NUM_XO).map(|_| Biquad::new()).collect(),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.smoothing_factor = compute_smoothing_factor(sample_rate);
    }

    pub fn set_preamp_db(&mut self, preamp_db: f64) {
        let db = if preamp_db.is_finite() {
            preamp_db
        } else {
            0.0
        };
        self.target_preamp_gain = (10.0_f64).powf(db / 20.0);
    }

    pub fn set_crossover(&mut self, enabled: bool, xo_type: i32, gains: [f64; 5]) {
        self.xo_enabled = enabled;
        self.xo_type = xo_type;
        for i in 0..5 {
            self.xo_gains[i] = if gains[i].is_finite() { gains[i] } else { 1.0 };
        }
    }

    pub fn update_filter(
        &mut self,
        bank: Bank,
        index: usize,
        ftype: FilterType,
        freq: f64,
        gain: f64,
        q: f64,
        bypassed: bool,
    ) {
        let sr = self.sample_rate;
        let bank_vec = match bank {
            Bank::Eq => &mut self.filters,
            Bank::Sim => &mut self.sim_filters,
            Bank::Xo => &mut self.xo_filters,
        };
        if let Some(f) = bank_vec.get_mut(index) {
            let was_bypassed = f.bypassed;
            f.bypassed = bypassed;
            f.update_coefficients(ftype, freq, gain, q, sr, was_bypassed);
        }
    }

    pub fn reset(&mut self) {
        for f in self.filters.iter_mut() {
            f.reset();
        }
        for f in self.sim_filters.iter_mut() {
            f.reset();
        }
        for f in self.xo_filters.iter_mut() {
            f.reset();
        }
    }

    /// Process a stereo block in place-friendly form. Slices must all be `n` long.
    pub fn process(
        &mut self,
        in_l: &[f32],
        in_r: &[f32],
        out_l: &mut [f32],
        out_r: &mut [f32],
        n: usize,
    ) {
        let sf = self.smoothing_factor;
        let sr = self.sample_rate;
        for i in 0..n {
            self.preamp_gain += (self.target_preamp_gain - self.preamp_gain) * sf;

            let mut sample_l = in_l[i] as f64 * self.preamp_gain;
            let mut sample_r = in_r[i] as f64 * self.preamp_gain;

            // 1. Parametric EQ
            for f in self.filters.iter_mut() {
                if !f.bypassed {
                    sample_l = f.process_sample_l(sample_l, sf, sr);
                    sample_r = f.process_sample_r(sample_r);
                }
            }
            // 2. Acoustics & simulations
            for f in self.sim_filters.iter_mut() {
                if !f.bypassed {
                    sample_l = f.process_sample_l(sample_l, sf, sr);
                    sample_r = f.process_sample_r(sample_r);
                }
            }
            // 3. Active crossover (parallel branches summed with per-branch gains)
            if self.xo_enabled {
                let t = self.xo_type;
                let (mut sum_l, mut sum_r) = (0.0f64, 0.0f64);

                // branch 1 (always)
                let (mut b1l, mut b1r) = (sample_l, sample_r);
                if !self.xo_filters[0].bypassed {
                    b1l = self.xo_filters[0].process_sample_l(sample_l, sf, sr);
                    b1r = self.xo_filters[0].process_sample_r(sample_r);
                }
                sum_l += b1l * self.xo_gains[0];
                sum_r += b1r * self.xo_gains[0];

                // branch 2 (5-way only): xo[1] -> xo[2]
                if t == 5 {
                    let (mut l, mut r) = (sample_l, sample_r);
                    if !self.xo_filters[1].bypassed {
                        l = self.xo_filters[1].process_sample_l(sample_l, sf, sr);
                        l = self.xo_filters[2].process_sample_l(l, sf, sr);
                        r = self.xo_filters[1].process_sample_r(sample_r);
                        r = self.xo_filters[2].process_sample_r(r);
                    }
                    sum_l += l * self.xo_gains[1];
                    sum_r += r * self.xo_gains[1];
                }

                // branch 3 (3/4/5-way): xo[3] -> xo[4]
                if t == 3 || t == 4 || t == 5 {
                    let (mut l, mut r) = (sample_l, sample_r);
                    if !self.xo_filters[3].bypassed {
                        l = self.xo_filters[3].process_sample_l(sample_l, sf, sr);
                        l = self.xo_filters[4].process_sample_l(l, sf, sr);
                        r = self.xo_filters[3].process_sample_r(sample_r);
                        r = self.xo_filters[4].process_sample_r(r);
                    }
                    sum_l += l * self.xo_gains[2];
                    sum_r += r * self.xo_gains[2];
                }

                // branch 4 (4/5-way): xo[5] -> xo[6]
                if t == 4 || t == 5 {
                    let (mut l, mut r) = (sample_l, sample_r);
                    if !self.xo_filters[5].bypassed {
                        l = self.xo_filters[5].process_sample_l(sample_l, sf, sr);
                        l = self.xo_filters[6].process_sample_l(l, sf, sr);
                        r = self.xo_filters[5].process_sample_r(sample_r);
                        r = self.xo_filters[6].process_sample_r(r);
                    }
                    sum_l += l * self.xo_gains[3];
                    sum_r += r * self.xo_gains[3];
                }

                // branch 5 (always): xo[7]
                let (mut b5l, mut b5r) = (sample_l, sample_r);
                if !self.xo_filters[7].bypassed {
                    b5l = self.xo_filters[7].process_sample_l(sample_l, sf, sr);
                    b5r = self.xo_filters[7].process_sample_r(sample_r);
                }
                sum_l += b5l * self.xo_gains[4];
                sum_r += b5r * self.xo_gains[4];

                sample_l = sum_l;
                sample_r = sum_r;
            }

            out_l[i] = sample_l as f32;
            out_r[i] = sample_r as f32;
        }
    }
}
