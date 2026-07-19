//! IEM Manifest Generator (Rust port).
//! Scans the `data/` folder next to the executable (or a path given as arg 1)
//! and rewrites `manifest.json` so the app can detect the current curve library.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root: PathBuf = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let data_dir = root.join("data");
    if !data_dir.is_dir() {
        eprintln!("error: no 'data' folder found at {}", data_dir.display());
        return ExitCode::FAILURE;
    }
    match iem_utils::generate_manifest(&data_dir, "data") {
        Ok(entries) => {
            let json = iem_utils::manifest_to_json(&entries);
            let out = root.join("manifest.json");
            if let Err(e) = fs::write(&out, json) {
                eprintln!("error: could not write {}: {e}", out.display());
                return ExitCode::FAILURE;
            }
            println!("Wrote {} entries to {}", entries.len(), out.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error scanning data folder: {e}");
            ExitCode::FAILURE
        }
    }
}
