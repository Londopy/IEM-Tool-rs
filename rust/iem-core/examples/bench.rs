//! Zero-dependency DSP benchmark.
//!
//! Measures real-time engine throughput so performance work is grounded in
//! numbers rather than guesswork.
//!
//!     cargo run --release -p iem-core --example bench

use std::time::Instant;

use iem_core::biquad::FilterType;
use iem_core::engine::{Bank, EqEngine};

const SR: f64 = 48000.0;
const BLOCK: usize = 128; // WebAudio render quantum
const BLOCKS: usize = 20_000; // ~53 s of audio at 48 kHz

fn make_engine(active_eq: usize, active_sim: usize, crossover: bool) -> EqEngine {
    let mut e = EqEngine::new(SR);
    e.set_preamp_db(-3.0);
    for i in 0..active_eq {
        // spread bands across the spectrum
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
    if crossover {
        e.set_crossover(true, 3, [1.0; 5]);
        for i in [0usize, 3, 4, 7] {
            e.update_filter(Bank::Xo, i, FilterType::LowPass, 2000.0, 0.0, 0.707, false);
        }
    }
    e
}

fn bench(label: &str, mut e: EqEngine) {
    let input: Vec<f32> = (0..BLOCK).map(|i| (i as f32 * 0.05).sin() * 0.4).collect();
    let mut out_l = vec![0.0f32; BLOCK];
    let mut out_r = vec![0.0f32; BLOCK];

    // warm-up (let parameter interpolation settle, prime caches)
    for _ in 0..500 {
        e.process(&input, &input, &mut out_l, &mut out_r, BLOCK);
    }

    let start = Instant::now();
    for _ in 0..BLOCKS {
        e.process(&input, &input, &mut out_l, &mut out_r, BLOCK);
    }
    let elapsed = start.elapsed();

    let frames = (BLOCKS * BLOCK) as f64;
    let secs = elapsed.as_secs_f64();
    let ns_per_frame = secs * 1e9 / frames;
    let audio_seconds = frames / SR;
    let realtime_x = audio_seconds / secs;

    println!(
        "{label:<34} {ns_per_frame:>8.1} ns/frame   {realtime_x:>8.0}x realtime   ({:.3} s for {:.1} s audio)",
        secs, audio_seconds
    );

    // keep the optimiser honest
    std::hint::black_box(&out_l);
    std::hint::black_box(&out_r);
}

fn main() {
    println!("iem-core DSP benchmark  —  {BLOCK}-frame blocks @ {SR} Hz\n");
    bench("bypassed (0 filters)", make_engine(0, 0, false));
    bench("10 EQ bands", make_engine(10, 0, false));
    bench("32 EQ bands", make_engine(32, 0, false));
    bench("80 EQ bands (max)", make_engine(80, 0, false));
    bench("80 EQ + 15 sim", make_engine(80, 15, false));
    bench("80 EQ + 15 sim + crossover", make_engine(80, 15, true));
}
