//! IEM Tool — Tauri backend.
//!
//! Exposes the verified `iem-core` computational core and the `iem-utils`
//! helper utilities to the (unchanged) HTML/CSS/JS frontend as Tauri commands.
//! The frontend can call these via `@tauri-apps/api`'s `invoke(...)`, e.g.
//! `invoke('biquad_magnitude', { ftype: 0, f, f0, q, g, fs })`.

use std::path::PathBuf;

use iem_core::autoeq;
use iem_core::biquad::FilterType;
use iem_core::curves::{self, AlignMode};
use iem_core::magnitude;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Band {
    pub ftype: i32,
    pub f0: f64,
    pub q: f64,
    pub g: f64,
}

#[derive(Serialize)]
pub struct AutoEqOut {
    pub gains: Vec<f64>,
    pub preamp: f64,
}

#[tauri::command]
fn biquad_magnitude(ftype: i32, f: f64, f0: f64, q: f64, g: f64, fs: f64) -> f64 {
    magnitude::get_biquad_magnitude(FilterType::from_i32(ftype), f, f0, q, g, fs)
}

#[tauri::command]
fn chain_magnitude_db(bands: Vec<Band>, freqs: Vec<f64>, fs: f64) -> Vec<f64> {
    freqs
        .iter()
        .map(|&f| {
            bands.iter().fold(0.0, |acc, b| {
                let m = magnitude::get_biquad_magnitude(
                    FilterType::from_i32(b.ftype),
                    f,
                    b.f0,
                    b.q,
                    b.g,
                    fs,
                );
                acc + 20.0 * m.max(1e-10).log10()
            })
        })
        .collect()
}

#[tauri::command]
fn cubic_spline(points: Vec<(f64, f64)>, targets: Vec<f64>) -> Vec<f64> {
    curves::cubic_spline_interpolate(&points, &targets)
}

#[tauri::command]
fn gaussian_smooth(freqs: Vec<f64>, values: Vec<f64>, octave_bw: f64) -> Vec<f64> {
    curves::gaussian_smooth(&freqs, &values, octave_bw)
}

#[tauri::command]
fn normalize_to_75db(
    data: Vec<(f64, f64)>,
    mode_hz: Option<f64>,
    target_db: f64,
) -> Vec<(f64, f64)> {
    let mode = match mode_hz {
        Some(hz) => AlignMode::Hz(hz),
        None => AlignMode::Mean,
    };
    curves::normalize_to_75db(&data, mode, target_db)
}

#[tauri::command]
fn log_grid(num_points: usize) -> Vec<f64> {
    curves::generate_log_grid(num_points)
}

#[tauri::command]
fn autoeq_solve(
    target_correction: Vec<f64>,
    freqs: Vec<f64>,
    band_freqs: Vec<f64>,
    band_qs: Vec<f64>,
    fs: f64,
) -> AutoEqOut {
    let r = autoeq::solve(&target_correction, &freqs, &band_freqs, &band_qs, fs);
    AutoEqOut {
        gains: r.gains,
        preamp: r.preamp,
    }
}

#[tauri::command]
fn generate_manifest(root: String) -> Result<usize, String> {
    let root = PathBuf::from(root);
    let data_dir = root.join("data");
    let entries = iem_utils::generate_manifest(&data_dir, "data").map_err(|e| e.to_string())?;
    let json = iem_utils::manifest_to_json(&entries);
    std::fs::write(root.join("manifest.json"), json).map_err(|e| e.to_string())?;
    Ok(entries.len())
}

#[tauri::command]
fn convert_curves(dir: String) -> Result<usize, String> {
    // Thin wrapper around the same logic used by the standalone converter binary.
    use iem_utils::{
        average_pair, channel_of, list_raw_files, parse_measurement, points_to_text,
        standardize_name, Point,
    };
    use std::collections::BTreeMap;

    let dir = PathBuf::from(dir);
    let files = list_raw_files(&dir).map_err(|e| e.to_string())?;
    let mut groups: BTreeMap<String, Vec<(Option<char>, Vec<Point>)>> = BTreeMap::new();
    for path in &files {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let (base, chan) = channel_of(&stem);
        let body = std::fs::read_to_string(path).unwrap_or_default();
        let pts = parse_measurement(&body);
        if !pts.is_empty() {
            groups.entry(base).or_default().push((chan, pts));
        }
    }
    let out_dir = dir.join("Converted");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let mut written = 0;
    for (base, mut variants) in groups {
        let merged: Vec<Point> = if variants.len() >= 2 {
            variants.sort_by_key(|(c, _)| *c);
            average_pair(&variants[0].1, &variants[1].1)
        } else {
            variants.pop().map(|(_, p)| p).unwrap_or_default()
        };
        let name = standardize_name(&base);
        if std::fs::write(out_dir.join(format!("{name}.txt")), points_to_text(&merged)).is_ok() {
            written += 1;
        }
    }
    Ok(written)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            biquad_magnitude,
            chain_magnitude_db,
            cubic_spline,
            gaussian_smooth,
            normalize_to_75db,
            log_grid,
            autoeq_solve,
            generate_manifest,
            convert_curves
        ])
        .run(tauri::generate_context!())
        .expect("error while running IEM Tool");
}
