//! Native Rust reimplementations of the two IEM Tool helper utilities that
//! originally shipped as Python-built Windows executables:
//!   * `manifest`  — rebuild `manifest.json` from the `data/` curve library.
//!   * `converter` — normalize raw measurement exports (Squiglink/Crinacle/etc.)
//!     into the app's standard single-curve `.txt` format, averaging L/R pairs.
//!
//! No external crates: JSON is emitted by hand in the exact 2-space,
//! `{ "file": ..., "size": ... }` shape the app already ships and consumes.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

// =========================================================================
// Manifest generator
// =========================================================================

#[derive(Clone)]
pub struct ManifestEntry {
    /// Forward-slash path relative to the app-files root, e.g. `data/ADEN/X.txt`.
    pub file: String,
    /// Size in bytes.
    pub size: u64,
}

/// Recursively scan `data_dir` for `.txt` curve files and return manifest
/// entries sorted by path (matching the committed manifest ordering).
/// `rel_prefix` is prepended to each path (use "data" to match the app).
pub fn generate_manifest(data_dir: &Path, rel_prefix: &str) -> io::Result<Vec<ManifestEntry>> {
    let mut out = Vec::new();
    let mut stack = vec![data_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let ft = entry.file_type()?;
            if ft.is_dir() {
                stack.push(path);
            } else if ft.is_file()
                && path
                    .extension()
                    .map(|e| e.eq_ignore_ascii_case("txt"))
                    .unwrap_or(false)
            {
                let rel = path.strip_prefix(data_dir).unwrap();
                let rel_str = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                let file = if rel_prefix.is_empty() {
                    rel_str
                } else {
                    format!("{}/{}", rel_prefix.trim_end_matches('/'), rel_str)
                };
                let size = fs::metadata(&path)?.len();
                out.push(ManifestEntry { file, size });
            }
        }
    }
    out.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(out)
}

/// Serialize manifest entries to the exact JSON layout the app ships.
pub fn manifest_to_json(entries: &[ManifestEntry]) -> String {
    if entries.is_empty() {
        return "[]".to_string();
    }
    let mut s = String::from("[\n");
    for (i, e) in entries.iter().enumerate() {
        s.push_str("  {\n");
        s.push_str(&format!("    \"file\": {},\n", json_string(&e.file)));
        s.push_str(&format!("    \"size\": {}\n", e.size));
        s.push_str(if i + 1 < entries.len() {
            "  },\n"
        } else {
            "  }\n"
        });
    }
    s.push(']');
    s
}

/// Minimal JSON string escaper (handles the characters that occur in paths).
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

// =========================================================================
// Curve converter
// =========================================================================

/// A parsed measurement point.
pub type Point = (f64, f64);

/// Parse a raw measurement file body into (freq, db) points. Accepts space,
/// tab, semicolon or comma separation; skips headers/comment/blank lines.
pub fn parse_measurement(body: &str) -> Vec<Point> {
    let mut pts = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with('*')
            || line.starts_with('/')
        {
            continue;
        }
        let mut it = line
            .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .filter(|t| !t.is_empty());
        let (a, b) = (it.next(), it.next());
        if let (Some(a), Some(b)) = (a, b) {
            if let (Ok(f), Ok(d)) = (a.parse::<f64>(), b.parse::<f64>()) {
                if f.is_finite() && d.is_finite() {
                    pts.push((f, d));
                }
            }
        }
    }
    pts
}

/// Average two measurement curves (e.g. Left `[1]` and Right `[2]`) onto the
/// union of their frequency points using log-frequency linear interpolation.
pub fn average_pair(a: &[Point], b: &[Point]) -> Vec<Point> {
    if a.is_empty() {
        return b.to_vec();
    }
    if b.is_empty() {
        return a.to_vec();
    }
    // Use the first curve's frequency axis as the reference grid.
    a.iter()
        .map(|&(f, da)| {
            let db = interp_log(b, f);
            (f, (da + db) / 2.0)
        })
        .collect()
}

/// Log-frequency linear interpolation of a sorted curve at `f`.
fn interp_log(curve: &[Point], f: f64) -> f64 {
    if curve.is_empty() {
        return 0.0;
    }
    let last = curve.len() - 1;
    if f <= curve[0].0 {
        return curve[0].1;
    }
    if f >= curve[last].0 {
        return curve[last].1;
    }
    let mut lo = 0usize;
    // linear scan is fine; measurement files are small and monotonic.
    while lo < last && curve[lo + 1].0 < f {
        lo += 1;
    }
    let (x0, y0) = curve[lo];
    let (x1, y1) = curve[lo + 1];
    if x1 == x0 {
        return y0;
    }
    y0 + (f.log10() - x0.log10()) * (y1 - y0) / (x1.log10() - x0.log10())
}

/// Standardize a display/file name: strip channel markers and extension,
/// collapse whitespace, uppercase (matching the shipped library style).
pub fn standardize_name(raw: &str) -> String {
    let mut name = raw.to_string();
    // strip common channel markers
    for marker in [
        " [1]", " [2]", " L", " R", "_L", "_R", " (L)", " (R)", " Left", " Right",
    ] {
        if let Some(pos) = name.rfind(marker) {
            if pos + marker.len() == name.len() {
                name.truncate(pos);
            }
        }
    }
    let collapsed = name.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim().to_uppercase()
}

/// Serialize points back to the app's `freq db` text format (one pair per line).
pub fn points_to_text(points: &[Point]) -> String {
    let mut s = String::new();
    for &(f, d) in points {
        s.push_str(&format!("{} {}\n", trim_num(f), trim_num(d)));
    }
    s
}

fn trim_num(v: f64) -> String {
    // compact numeric formatting similar to the source files
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// Detect the channel-pair base name and channel of a file stem, if any.
/// Returns (base, channel) where channel is Some('1'|'2') for [1]/[2] variants.
pub fn channel_of(stem: &str) -> (String, Option<char>) {
    if let Some(base) = stem.strip_suffix(" [1]") {
        return (base.to_string(), Some('1'));
    }
    if let Some(base) = stem.strip_suffix(" [2]") {
        return (base.to_string(), Some('2'));
    }
    (stem.to_string(), None)
}

/// Collect `.txt`/`.csv` files in `dir` (non-recursive).
pub fn list_raw_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut v = Vec::new();
    for entry in fs::read_dir(dir)? {
        let p = entry?.path();
        if p.is_file() {
            if let Some(ext) = p.extension() {
                if ext.eq_ignore_ascii_case("txt") || ext.eq_ignore_ascii_case("csv") {
                    v.push(p);
                }
            }
        }
    }
    v.sort();
    Ok(v)
}

// =========================================================================
// Parametric EQ  ->  Graphic EQ exporter
// =========================================================================

/// Convert a standard **ParametricEQ** description (the widely used AutoEq
/// `Preamp:` + `Filter N: ON PK Fc ... Gain ... Q ...` text format) into a
/// **GraphicEQ** correction curve, by evaluating the combined biquad response
/// on a dense frequency grid using the verified `iem-core` DSP.
///
/// Output is the common interchange format consumed by many equalizer apps
/// (e.g. a `GraphicEQ: f g; f g; ...` one-liner, or plain `freq gain` lines).
/// This module is intentionally agnostic to any particular consumer.
pub mod graphiceq {
    use iem_core::biquad::FilterType;
    // Use the RBJ-corrected magnitude: the exporter needs a physically accurate
    // response, including for high-shelf filters (the faithful/plot variant
    // reproduces the original app's high-shelf sign quirk, wrong for real curves).
    use iem_core::magnitude::get_biquad_magnitude_rbj as get_biquad_magnitude;

    /// One parametric band.
    #[derive(Clone, Copy, Debug)]
    pub struct PeqBand {
        pub ftype: FilterType,
        pub fc: f64,
        pub q: f64,
        pub gain_db: f64,
    }

    /// A parsed parametric EQ: an optional global pre-amp and a list of bands.
    #[derive(Clone, Debug, Default)]
    pub struct ParametricEq {
        pub preamp_db: Option<f64>,
        pub bands: Vec<PeqBand>,
    }

    /// Map a filter-type token (AutoEq / REW style) to a `FilterType`.
    pub fn filter_type_from_token(t: &str) -> Option<FilterType> {
        match t.to_ascii_uppercase().as_str() {
            "PK" | "PEQ" | "PEAKING" | "MODAL" | "BELL" => Some(FilterType::Peaking),
            "LS" | "LSC" | "LSQ" | "LOWSHELF" => Some(FilterType::LowShelf),
            "HS" | "HSC" | "HSQ" | "HIGHSHELF" => Some(FilterType::HighShelf),
            "LP" | "LPQ" | "LOWPASS" => Some(FilterType::LowPass),
            "HP" | "HPQ" | "HIGHPASS" => Some(FilterType::HighPass),
            "NO" | "NOTCH" => Some(FilterType::Notch),
            _ => None,
        }
    }

    /// Parse ParametricEQ text. Recognizes `Preamp: <db> dB` and
    /// `Filter N: ON <TYPE> Fc <hz> Hz Gain <db> dB Q <q>` lines; `OFF`
    /// filters and unparseable lines are skipped.
    pub fn parse_parametric_eq(text: &str) -> ParametricEq {
        let mut eq = ParametricEq::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line
                .strip_prefix("Preamp:")
                .or_else(|| line.strip_prefix("Preamp"))
            {
                if let Some(v) = first_float(rest) {
                    eq.preamp_db = Some(v);
                }
                continue;
            }
            if line.starts_with("Filter") {
                let toks: Vec<&str> = line.split_whitespace().collect();
                if toks.iter().any(|t| t.eq_ignore_ascii_case("OFF")) {
                    continue;
                }
                // Type token: the one right after "ON".
                let ftype = toks
                    .iter()
                    .position(|t| t.eq_ignore_ascii_case("ON"))
                    .and_then(|i| toks.get(i + 1))
                    .and_then(|t| filter_type_from_token(t));
                let ftype = match ftype {
                    Some(f) => f,
                    None => continue,
                };
                let fc = value_after(&toks, "Fc");
                let gain = value_after(&toks, "Gain").unwrap_or(0.0);
                let q = value_after(&toks, "Q").unwrap_or(0.707);
                if let Some(fc) = fc {
                    eq.bands.push(PeqBand {
                        ftype,
                        fc,
                        q,
                        gain_db: gain,
                    });
                }
            }
        }
        eq
    }

    fn first_float(s: &str) -> Option<f64> {
        s.split_whitespace().find_map(|t| t.parse::<f64>().ok())
    }

    /// Value of the token immediately following `key` (parsed as f64).
    fn value_after(toks: &[&str], key: &str) -> Option<f64> {
        toks.iter()
            .position(|t| t.eq_ignore_ascii_case(key))
            .and_then(|i| toks.get(i + 1))
            .and_then(|t| t.parse::<f64>().ok())
    }

    /// `n` log-spaced frequencies from `fmin` to `fmax` (inclusive).
    pub fn log_grid(n: usize, fmin: f64, fmax: f64) -> Vec<f64> {
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            let t = if n > 1 {
                i as f64 / (n - 1) as f64
            } else {
                0.0
            };
            v.push(fmin * (fmax / fmin).powf(t));
        }
        v
    }

    /// Combined magnitude response (dB) of all bands at each frequency.
    pub fn response_db(bands: &[PeqBand], freqs: &[f64], fs: f64) -> Vec<f64> {
        freqs
            .iter()
            .map(|&f| {
                bands.iter().fold(0.0, |acc, b| {
                    let m = get_biquad_magnitude(b.ftype, f, b.fc, b.q, b.gain_db, fs);
                    acc + 20.0 * m.max(1e-10).log10()
                })
            })
            .collect()
    }

    /// Options for building a GraphicEQ curve.
    pub struct Options {
        pub points: usize,
        pub fmin: f64,
        pub fmax: f64,
        pub fs: f64,
        /// Normalize so the loudest point is 0 dB (pure attenuation, no clipping).
        pub normalize_peak: bool,
        /// Optional explicit global offset (dB) applied when not normalizing.
        pub preamp_db: Option<f64>,
        /// Optional per-point clamp (e.g. Some(12.0) for +/-12 dB).
        pub clamp_db: Option<f64>,
    }

    impl Default for Options {
        fn default() -> Self {
            Options {
                points: 128,
                fmin: 20.0,
                fmax: 20000.0,
                fs: 48000.0,
                normalize_peak: true,
                preamp_db: None,
                clamp_db: None,
            }
        }
    }

    /// Result of a conversion: the grid, the gains, and the effective offset used.
    pub struct GraphicEq {
        pub freqs: Vec<f64>,
        pub gains_db: Vec<f64>,
        /// The global dB offset that was applied (baked headroom or explicit preamp).
        pub applied_offset_db: f64,
        /// How many points were clamped (if clamping enabled).
        pub clamped: usize,
    }

    /// Convert parametric bands into a GraphicEQ curve per `opts`.
    pub fn build(bands: &[PeqBand], opts: &Options) -> GraphicEq {
        let freqs = log_grid(opts.points, opts.fmin, opts.fmax);
        let mut gains = response_db(bands, &freqs, opts.fs);

        let offset = if opts.normalize_peak {
            let peak = gains
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max)
                .max(0.0);
            -peak
        } else {
            opts.preamp_db.unwrap_or(0.0)
        };
        for g in gains.iter_mut() {
            *g += offset;
        }

        let mut clamped = 0;
        if let Some(c) = opts.clamp_db {
            for g in gains.iter_mut() {
                if *g > c {
                    *g = c;
                    clamped += 1;
                } else if *g < -c {
                    *g = -c;
                    clamped += 1;
                }
            }
        }

        GraphicEq {
            freqs,
            gains_db: gains,
            applied_offset_db: offset,
            clamped,
        }
    }

    /// Format as the single-line `GraphicEQ: f g; f g; ...` interchange format.
    pub fn format_graphiceq_line(g: &GraphicEq) -> String {
        let body: Vec<String> = g
            .freqs
            .iter()
            .zip(g.gains_db.iter())
            .map(|(f, gain)| format!("{} {:.1}", f.round() as i64, gain))
            .collect();
        format!("GraphicEQ: {}", body.join("; "))
    }

    /// Format as plain `freq gain` lines (one pair per line).
    pub fn format_pairs(g: &GraphicEq) -> String {
        let mut s = String::new();
        for (f, gain) in g.freqs.iter().zip(g.gains_db.iter()) {
            s.push_str(&format!("{} {:.1}\n", f.round() as i64, gain));
        }
        s
    }
}
