//! Integration tests for the IEM Tool Rust core. These run in CI via `cargo test`.
use iem_core::autoeq;
use iem_core::biquad::{self, Biquad, FilterType};
use iem_core::curves::{self, AlignMode};
use iem_core::engine::{Bank, EqEngine};
use iem_core::magnitude::get_biquad_magnitude;

const FS: f64 = 48000.0;

fn approx(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn peaking_zero_gain_is_unity() {
    // With G = 0 a peaking/shelf filter must be exactly transparent.
    for &t in &[
        FilterType::Peaking,
        FilterType::LowShelf,
        FilterType::HighShelf,
    ] {
        for &f in &[50.0, 500.0, 5000.0, 15000.0] {
            let m = get_biquad_magnitude(t, f, 1000.0, 1.0, 0.0, FS);
            assert!(approx(m, 1.0, 1e-12), "expected unity, got {m}");
        }
    }
}

#[test]
fn peaking_gain_at_center_matches_db() {
    // At f == f0 a peaking filter's magnitude in dB should equal its gain.
    for &g in &[-12.0, -6.0, 3.0, 9.0] {
        let m = get_biquad_magnitude(FilterType::Peaking, 1000.0, 1000.0, 2.0, g, FS);
        let db = 20.0 * m.log10();
        assert!(approx(db, g, 1e-6), "center gain {g} dB, got {db} dB");
    }
}

#[test]
fn lowpass_passes_lows_blocks_highs() {
    let low = get_biquad_magnitude(FilterType::LowPass, 100.0, 1000.0, 0.707, 0.0, FS);
    let high = get_biquad_magnitude(FilterType::LowPass, 18000.0, 1000.0, 0.707, 0.0, FS);
    assert!(low > 0.9, "lowpass should pass lows, got {low}");
    assert!(high < 0.1, "lowpass should block highs, got {high}");
}

#[test]
fn design_coeffs_are_finite() {
    for &t in &[
        FilterType::Peaking,
        FilterType::LowShelf,
        FilterType::HighShelf,
        FilterType::LowPass,
        FilterType::HighPass,
        FilterType::Notch,
    ] {
        let c = biquad::design(t, 1000.0, 6.0, 1.0, FS);
        for v in [c.b0, c.b1, c.b2, c.a1, c.a2] {
            assert!(v.is_finite(), "non-finite coefficient");
        }
    }
}

#[test]
fn highshelf_rbj_variant_differs_from_faithful() {
    // The corrected RBJ high-shelf should differ from the faithful (buggy) one.
    let faithful = iem_core::magnitude::get_biquad_magnitude(
        FilterType::HighShelf,
        8000.0,
        6000.0,
        0.7,
        6.0,
        FS,
    );
    let rbj = iem_core::magnitude::get_biquad_magnitude_rbj(
        FilterType::HighShelf,
        8000.0,
        6000.0,
        0.7,
        6.0,
        FS,
    );
    assert!((faithful - rbj).abs() > 1e-6, "variants should differ");
}

#[test]
fn spline_passes_through_knots() {
    let pts = vec![(20.0, 70.0), (200.0, 75.0), (2000.0, 72.0), (20000.0, 68.0)];
    let targets: Vec<f64> = pts.iter().map(|p| p.0).collect();
    let out = curves::cubic_spline_interpolate(&pts, &targets);
    for (i, p) in pts.iter().enumerate() {
        assert!(approx(out[i], p.1, 1e-6), "knot {i}: {} vs {}", out[i], p.1);
    }
}

#[test]
fn gaussian_smooth_preserves_constant() {
    let freqs = curves::generate_log_grid(200);
    let values = vec![75.0; freqs.len()];
    let out = curves::gaussian_smooth(&freqs, &values, 0.08);
    for v in out {
        assert!(approx(v, 75.0, 1e-9), "constant not preserved: {v}");
    }
}

#[test]
fn log_grid_endpoints() {
    let g = curves::generate_log_grid(500);
    assert!(approx(g[0], 20.0, 1e-9));
    assert!(approx(g[499], 20000.0, 1e-6));
    // strictly increasing
    for w in g.windows(2) {
        assert!(w[1] > w[0]);
    }
}

#[test]
fn normalize_aligns_reference_point() {
    let data = vec![(500.0, 60.0), (1000.0, 65.0), (2000.0, 62.0)];
    let out = curves::normalize_to_75db(&data, AlignMode::Hz(1000.0), 75.0);
    // The 1000 Hz point must become exactly 75 dB.
    let p = out.iter().find(|p| p.0 == 1000.0).unwrap();
    assert!(approx(p.1, 75.0, 1e-9));
}

#[test]
fn average_of_identical_curves_is_stable() {
    let c = vec![(20.0, 70.0), (1000.0, 75.0), (20000.0, 68.0)];
    let curves_in = vec![c.clone(), c.clone(), c.clone(), c.clone()];
    let grid = curves::generate_log_grid(100);
    let avg = curves::average_curves(&curves_in, &grid, AlignMode::Hz(1000.0), 75.0);
    let single = {
        let n = curves::normalize_to_75db(&c, AlignMode::Hz(1000.0), 75.0);
        curves::cubic_spline_interpolate(&n, &grid)
    };
    // averaging identical curves then smoothing stays close to the (smoothed) curve
    let smoothed_single = curves::gaussian_smooth(&grid, &single, 0.05);
    for i in 0..grid.len() {
        assert!(approx(avg[i], smoothed_single[i], 1e-6));
    }
}

#[test]
fn autoeq_zero_target_gives_zero_gains() {
    let freqs = curves::generate_log_grid(200);
    let target = vec![0.0; freqs.len()];
    let bf: Vec<f64> = (0..10)
        .map(|i| 20.0 * 950f64.powf(i as f64 / 9.0))
        .collect();
    let bq = vec![2.0; 10];
    let r = autoeq::solve(&target, &freqs, &bf, &bq, FS);
    for g in &r.gains {
        assert!(approx(*g, 0.0, 1e-9));
    }
    assert!(approx(r.preamp, 0.0, 1e-9));
}

#[test]
fn autoeq_recovers_single_band() {
    // Target correction that is exactly +6 dB of one band's response should be
    // recovered as ~+6 dB on that band.
    let freqs = curves::generate_log_grid(300);
    let bf = vec![1000.0];
    let bq = vec![1.5];
    let target: Vec<f64> = freqs
        .iter()
        .map(|&f| {
            6.0 * 20.0
                * get_biquad_magnitude(FilterType::Peaking, f, 1000.0, 1.5, 1.0, FS)
                    .max(1e-10)
                    .log10()
        })
        .collect();
    let r = autoeq::solve(&target, &freqs, &bf, &bq, FS);
    assert!(approx(r.gains[0], 6.0, 0.5), "recovered {}", r.gains[0]);
}

#[test]
fn engine_bypassed_is_preamp_only() {
    let mut e = EqEngine::new(FS);
    e.set_preamp_db(0.0);
    let inp: Vec<f32> = (0..256).map(|i| (i as f32 * 0.03).sin() * 0.4).collect();
    let mut ol = vec![0.0f32; 256];
    let mut or = vec![0.0f32; 256];
    e.process(&inp, &inp, &mut ol, &mut or, 256);
    for i in 0..256 {
        assert!(approx(ol[i] as f64, inp[i] as f64, 1e-6));
    }
}

#[test]
fn engine_is_deterministic() {
    let make = || {
        let mut e = EqEngine::new(FS);
        e.set_preamp_db(-3.0);
        e.update_filter(Bank::Eq, 0, FilterType::Peaking, 1000.0, 6.0, 1.0, false);
        e
    };
    let inp: Vec<f32> = (0..512).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
    let run = |mut e: EqEngine| {
        let (mut ol, mut or) = (vec![0.0f32; 512], vec![0.0f32; 512]);
        e.process(&inp, &inp, &mut ol, &mut or, 512);
        ol
    };
    let a = run(make());
    let b = run(make());
    assert_eq!(a, b, "engine must be deterministic");
    // output must be finite
    for v in &a {
        assert!(v.is_finite());
    }
}
