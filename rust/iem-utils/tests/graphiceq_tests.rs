//! Neutral tests for the ParametricEQ -> GraphicEQ exporter. Uses only synthetic
//! filter data (no device- or product-specific content).
use iem_core::biquad::FilterType;
use iem_utils::graphiceq::*;

fn approx(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn parses_preamp_filters_and_skips_off_and_comments() {
    let text = "\
# a synthetic parametric spec
Preamp: -6.5 dB
Filter 1: ON PK Fc 105 Hz Gain -3.0 dB Q 0.70
Filter 2: OFF PK Fc 500 Hz Gain 2.0 dB Q 1.00
Filter 3: ON LSC Fc 100 Hz Gain 4.0 dB Q 0.71
Filter 4: ON HSC Fc 8000 Hz Gain -2.0 dB Q 0.71
";
    let eq = parse_parametric_eq(text);
    assert_eq!(eq.preamp_db, Some(-6.5));
    assert_eq!(eq.bands.len(), 3, "the OFF filter must be skipped");
    assert!(matches!(eq.bands[0].ftype, FilterType::Peaking));
    assert!(approx(eq.bands[0].fc, 105.0, 1e-9));
    assert!(approx(eq.bands[0].gain_db, -3.0, 1e-9));
    assert!(approx(eq.bands[0].q, 0.70, 1e-9));
    assert!(matches!(eq.bands[1].ftype, FilterType::LowShelf));
    assert!(matches!(eq.bands[2].ftype, FilterType::HighShelf));
}

#[test]
fn empty_input_has_no_bands() {
    let eq = parse_parametric_eq("nothing to see here\nPreamp: 0 dB\n");
    assert!(eq.bands.is_empty());
}

#[test]
fn response_at_center_equals_gain() {
    // A single peaking filter reads its gain (dB) at its center frequency.
    let bands = [PeqBand {
        ftype: FilterType::Peaking,
        fc: 1000.0,
        q: 1.5,
        gain_db: 6.0,
    }];
    let r = response_db(&bands, &[1000.0], 48000.0);
    assert!(approx(r[0], 6.0, 1e-6), "got {}", r[0]);
}

#[test]
fn normalize_puts_peak_at_zero() {
    let bands = [PeqBand {
        ftype: FilterType::Peaking,
        fc: 1000.0,
        q: 1.0,
        gain_db: 6.0,
    }];
    let opts = Options {
        points: 128,
        normalize_peak: true,
        ..Options::default()
    };
    let g = build(&bands, &opts);
    let max = g.gains_db.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    assert!(approx(max, 0.0, 1e-9), "peak should be 0 dB, got {max}");
    assert!(
        approx(g.applied_offset_db, -6.0, 0.2),
        "offset ~ -6 dB, got {}",
        g.applied_offset_db
    );
    // every point is attenuation (<= 0)
    assert!(g.gains_db.iter().all(|&v| v <= 1e-9));
}

#[test]
fn no_bands_is_flat_zero() {
    let opts = Options::default();
    let g = build(&[], &opts);
    assert!(g.gains_db.iter().all(|&v| approx(v, 0.0, 1e-9)));
}

#[test]
fn clamp_limits_extremes() {
    let bands = [PeqBand {
        ftype: FilterType::Peaking,
        fc: 2000.0,
        q: 3.0,
        gain_db: 20.0,
    }];
    let opts = Options {
        normalize_peak: false,
        preamp_db: Some(0.0),
        clamp_db: Some(12.0),
        ..Options::default()
    };
    let g = build(&bands, &opts);
    assert!(g
        .gains_db
        .iter()
        .all(|&v| v <= 12.0 + 1e-9 && v >= -12.0 - 1e-9));
    assert!(g.clamped > 0, "at least one point should have been clamped");
}

#[test]
fn output_formats_are_well_formed() {
    let bands = [PeqBand {
        ftype: FilterType::Peaking,
        fc: 1000.0,
        q: 1.0,
        gain_db: 3.0,
    }];
    let opts = Options {
        points: 16,
        ..Options::default()
    };
    let g = build(&bands, &opts);

    let line = format_graphiceq_line(&g);
    assert!(line.starts_with("GraphicEQ: "));
    assert_eq!(line.matches(';').count(), 15, "16 points -> 15 separators");

    let pairs = format_pairs(&g);
    assert_eq!(pairs.lines().count(), 16);
    // each line is "int float"
    for l in pairs.lines() {
        let mut it = l.split_whitespace();
        assert!(it.next().unwrap().parse::<i64>().is_ok());
        assert!(it.next().unwrap().parse::<f64>().is_ok());
    }
}

#[test]
fn high_shelf_is_physically_correct() {
    // High shelf -4 dB @ 8k: ~0 dB well below the corner, ~-4 dB well above.
    let bands = [PeqBand {
        ftype: FilterType::HighShelf,
        fc: 8000.0,
        q: 0.707,
        gain_db: -4.0,
    }];
    let low = response_db(&bands, &[100.0], 48000.0)[0];
    let high = response_db(&bands, &[19000.0], 48000.0)[0];
    assert!(
        approx(low, 0.0, 0.5),
        "shelf should be ~0 dB in the bass, got {low}"
    );
    assert!(
        approx(high, -4.0, 0.6),
        "shelf should be ~-4 dB up top, got {high}"
    );
}

#[test]
fn low_shelf_is_physically_correct() {
    // Low shelf +5 dB @ 100: ~+5 dB in the deep bass, ~0 dB up high.
    let bands = [PeqBand {
        ftype: FilterType::LowShelf,
        fc: 100.0,
        q: 0.707,
        gain_db: 5.0,
    }];
    let low = response_db(&bands, &[25.0], 48000.0)[0];
    let high = response_db(&bands, &[10000.0], 48000.0)[0];
    assert!(approx(low, 5.0, 0.6), "got {low}");
    assert!(approx(high, 0.0, 0.5), "got {high}");
}
