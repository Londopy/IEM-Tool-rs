//! Deterministic output fingerprint, used to prove refactors stay bit-exact.
//!     cargo run --release -p iem-core --example dump
use iem_core::biquad::FilterType;
use iem_core::engine::{Bank, EqEngine};

const SR: f64 = 48000.0;
const BLOCK: usize = 128;

fn fingerprint(label: &str, active_eq: usize, active_sim: usize, xo: bool) {
    let mut e = EqEngine::new(SR);
    e.set_preamp_db(-3.0);
    for i in 0..active_eq {
        let f = 20.0 * (1000f64).powf(i as f64 / (active_eq.max(2) - 1) as f64);
        e.update_filter(Bank::Eq, i, FilterType::Peaking, f, 3.0, 1.0, false);
    }
    for i in 0..active_sim {
        e.update_filter(
            Bank::Sim,
            i,
            FilterType::HighShelf,
            6000.0,
            -2.0,
            0.707,
            false,
        );
    }
    if xo {
        e.set_crossover(true, 3, [1.0, 0.9, 0.8, 0.7, 0.6]);
        for i in [0usize, 3, 4, 7] {
            e.update_filter(Bank::Xo, i, FilterType::LowPass, 2000.0, 0.0, 0.707, false);
        }
    }

    // deterministic pseudo-random-ish input, several blocks so interpolation evolves
    let mut acc_l: u64 = 1469598103934665603;
    let mut acc_r: u64 = 1469598103934665603;
    let mut ol = vec![0.0f32; BLOCK];
    let mut or = vec![0.0f32; BLOCK];
    for b in 0..40 {
        let inp: Vec<f32> = (0..BLOCK)
            .map(|i| {
                let t = (b * BLOCK + i) as f32;
                (t * 0.031).sin() * 0.4 + (t * 0.007).cos() * 0.2
            })
            .collect();
        let inr: Vec<f32> = inp.iter().map(|v| v * 0.8).collect();
        e.process(&inp, &inr, &mut ol, &mut or, BLOCK);
        // FNV-1a over the raw output bits
        for v in &ol {
            acc_l ^= v.to_bits() as u64;
            acc_l = acc_l.wrapping_mul(1099511628211);
        }
        for v in &or {
            acc_r ^= v.to_bits() as u64;
            acc_r = acc_r.wrapping_mul(1099511628211);
        }
    }
    println!("{label:<30} L={acc_l:016x} R={acc_r:016x}");
}

fn main() {
    fingerprint("bypassed", 0, 0, false);
    fingerprint("10 eq", 10, 0, false);
    fingerprint("80 eq", 80, 0, false);
    fingerprint("80 eq + 15 sim", 80, 15, false);
    fingerprint("80 eq + 15 sim + xo", 80, 15, true);
}
