//! iem-autoeq-to-graphiceq — convert a ParametricEQ file (AutoEq `Preamp:` +
//! `Filter N: ON PK Fc ... Gain ... Q ...` format) into a GraphicEQ correction
//! curve. Output is a standard interchange format many equalizer apps accept.
//!
//! Usage:
//!   iem-autoeq-to-graphiceq <input.txt|-> [options]
//! Options:
//!   -o <file>        write output here (default: stdout)
//!   --pairs          emit plain "freq gain" lines (default: "GraphicEQ:" one-liner)
//!   --points <N>     number of grid points (default 128)
//!   --fs <hz>        sample rate for response eval (default 48000)
//!   --clamp <db>     clamp each point to +/- <db>
//!   --no-normalize   don't normalize the peak to 0 dB
//!   --preamp <db>    explicit global offset when not normalizing

use std::io::{Read, Write};
use std::process::ExitCode;

use iem_utils::graphiceq::{
    build, format_graphiceq_line, format_pairs, parse_parametric_eq, Options,
};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        eprintln!("usage: iem-autoeq-to-graphiceq <input.txt|-> [-o out] [--pairs] [--points N] [--fs HZ] [--clamp DB] [--no-normalize] [--preamp DB]");
        return if args.is_empty() {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        };
    }

    let input = &args[0];
    let mut out_path: Option<String> = None;
    let mut pairs = false;
    let mut opts = Options::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                out_path = args.get(i).cloned();
            }
            "--pairs" => pairs = true,
            "--line" => pairs = false,
            "--no-normalize" => opts.normalize_peak = false,
            "--points" => {
                i += 1;
                if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                    opts.points = v;
                }
            }
            "--fs" => {
                i += 1;
                if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                    opts.fs = v;
                }
            }
            "--clamp" => {
                i += 1;
                opts.clamp_db = args.get(i).and_then(|s| s.parse().ok());
            }
            "--preamp" => {
                i += 1;
                opts.preamp_db = args.get(i).and_then(|s| s.parse().ok());
                opts.normalize_peak = false;
            }
            other => {
                eprintln!("warning: ignoring unknown argument '{other}'");
            }
        }
        i += 1;
    }

    // Read input (file or stdin).
    let text = if input == "-" {
        let mut s = String::new();
        if std::io::stdin().read_to_string(&mut s).is_err() {
            eprintln!("error: could not read stdin");
            return ExitCode::FAILURE;
        }
        s
    } else {
        match std::fs::read_to_string(input) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: could not read {input}: {e}");
                return ExitCode::FAILURE;
            }
        }
    };

    let eq = parse_parametric_eq(&text);
    if eq.bands.is_empty() {
        eprintln!(
            "error: no filters found in input (expected ParametricEQ 'Filter N: ON ...' lines)"
        );
        return ExitCode::FAILURE;
    }

    let result = build(&eq.bands, &opts);
    let body = if pairs {
        format_pairs(&result)
    } else {
        format!("{}\n", format_graphiceq_line(&result))
    };

    // Diagnostics go to stderr so stdout stays clean for piping.
    eprintln!(
        "converted {} filter(s) -> {} points; applied offset {:.1} dB{}",
        eq.bands.len(),
        result.freqs.len(),
        result.applied_offset_db,
        if result.clamped > 0 {
            format!("; clamped {} point(s)", result.clamped)
        } else {
            String::new()
        },
    );

    match out_path {
        Some(p) => {
            if let Err(e) = std::fs::write(&p, body) {
                eprintln!("error: could not write {p}: {e}");
                return ExitCode::FAILURE;
            }
            eprintln!("wrote {p}");
        }
        None => {
            let _ = std::io::stdout().write_all(body.as_bytes());
        }
    }
    ExitCode::SUCCESS
}
