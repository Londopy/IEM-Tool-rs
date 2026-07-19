# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 2026-07-19

First release of the Rust port, built on the IEM Tool frontend by MyLittlePrimordia.

### Added
- `iem-core` — the computational core rewritten in Rust: RBJ biquad design, a
  stereo parameter-interpolating filter, the real-time engine (pre-amp, 80-band
  parametric EQ, 15-band simulation chain, 3/4/5-way crossover), frequency-response
  magnitude evaluation, curve utilities (log grid, normalization, cubic spline,
  Gaussian smoothing, trimmed-mean averaging) and the coordinate-descent AutoEQ solver.
- Dual build from one source: native `rlib` for the desktop backend, and
  WebAssembly (`app-files/wasm/iem_core.wasm`) for the browser audio thread.
- `dsp-processor-wasm.js` — a WebAssembly-backed AudioWorklet, verified
  sample-for-sample identical to the original JavaScript worklet.
- `iem-core.js` — main-thread bridge exposing the Rust core to the frontend.
- `iem-utils` — native Rust replacements for the two helper tools
  (`iem-manifest-generator`, `iem-curve-converter`), plus a new
  `iem-autoeq-to-graphiceq` exporter that converts a ParametricEQ file into a
  GraphicEQ correction curve.
- Tauri v2 desktop shell serving the existing frontend.
- 29 Rust tests and a cross-platform CI workflow (Windows, macOS, Linux).
- Release workflow producing installers for Windows (64- and 32-bit),
  macOS (Apple Silicon) and Linux, each published with SHA-256
  checksums and a build-provenance attestation.
- Optional helper scripts: `install.ps1` (checksum-verified download),
  `tools/build.ps1`, and `tools/Create-Shortcut.ps1`.

### Changed
- Desktop shell moved from Electron to Tauri.
- All number-crunching moved from JavaScript to Rust, verified against the
  original implementation (biquad magnitude to ~1e-13, AutoEQ to ~1e-7, and the
  audio engine bit-for-bit identical).

### Fixed
- Cubic-spline interpolation could read past its coefficient arrays when a target
  frequency landed exactly on an interior knot (silently produced `NaN` in the
  original); it now returns the correct knot value.
- The GraphicEQ exporter uses a corrected RBJ high-shelf so shelf filters are
  physically accurate. The original plotting routine's high-shelf `a1` sign quirk
  is still reproduced faithfully in `get_biquad_magnitude` for plot parity, with
  `get_biquad_magnitude_rbj` available as the corrected variant.

[Unreleased]: https://github.com/Londopy/IEM-Tool-rs/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/Londopy/IEM-Tool-rs/releases/tag/v1.0.0
