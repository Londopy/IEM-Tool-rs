//! IEM Curve Converter (Rust port).
//! Reads raw `.txt`/`.csv` measurement files from the current folder (or arg 1),
//! averages Left `[1]` / Right `[2]` pairs, standardizes names, and writes the
//! results into a `Converted/` sub-folder in the app's `freq db` text format.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use iem_utils::{
    average_pair, channel_of, list_raw_files, parse_measurement, points_to_text, standardize_name,
    Point,
};

fn main() -> ExitCode {
    let dir: PathBuf = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let files = match list_raw_files(&dir) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error reading {}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
    };
    if files.is_empty() {
        eprintln!(
            "No .txt or .csv measurement files found in {}",
            dir.display()
        );
        return ExitCode::FAILURE;
    }

    // Group by channel-pair base name.
    let mut groups: BTreeMap<String, Vec<(Option<char>, Vec<Point>)>> = BTreeMap::new();
    for path in &files {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let (base, chan) = channel_of(&stem);
        let body = fs::read_to_string(path).unwrap_or_default();
        let pts = parse_measurement(&body);
        if pts.is_empty() {
            continue;
        }
        groups.entry(base).or_default().push((chan, pts));
    }

    let out_dir = dir.join("Converted");
    if let Err(e) = fs::create_dir_all(&out_dir) {
        eprintln!("error creating {}: {e}", out_dir.display());
        return ExitCode::FAILURE;
    }

    let mut written = 0;
    for (base, mut variants) in groups {
        let merged: Vec<Point> = if variants.len() >= 2 {
            // average the first two channel variants (typically [1] and [2])
            variants.sort_by_key(|(c, _)| *c);
            average_pair(&variants[0].1, &variants[1].1)
        } else {
            variants.pop().map(|(_, p)| p).unwrap_or_default()
        };
        let name = standardize_name(&base);
        let out_path = out_dir.join(format!("{name}.txt"));
        if fs::write(&out_path, points_to_text(&merged)).is_ok() {
            written += 1;
        }
    }
    println!("Converted {written} curve(s) into {}", out_dir.display());
    ExitCode::SUCCESS
}
