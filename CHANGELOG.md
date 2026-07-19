# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.3.0] - 2026-07-19

Naming and versioning cleanup. No functional changes to the DSP or the app.

### Changed
- **Renamed the application to `IEM-Tool-rs` throughout** — product name, window
  title, installer filenames, crate names, Electron `productName`/shortcut, and
  the GitHub Release title. Installers are now `IEM-Tool-rs_<version>_…` with no
  space in the filename. The bundle identifier also moved from
  `com.mylittleprimordia.iemtool` to `com.londopy.iem-tool-rs`, since this is a
  separate application from the upstream project; the Tauri crate's `authors`
  was corrected to match. References to the *original* IEM Tool are unchanged.

### Fixed
- Installer filenames and the app's reported version were pinned at `1.0.0`
  regardless of the release tag, because Tauri reads the version from
  `tauri.conf.json` rather than from git. The release workflow now derives it
  from the tag before building, so installers always match the release.

## [1.2.0] - 2026-07-19

Supersedes 1.0.0 and 1.1.0, which have been withdrawn — their installers bundled
the upstream prebuilt `.exe` utilities described below.

### Added
- The command-line utilities (`iem-manifest-generator`, `iem-curve-converter`,
  `iem-autoeq-to-graphiceq`) are now **built for every platform and attached to
  each release**, so using them no longer requires a local Rust toolchain. They
  are covered by the release SHA-256 checksums and the build-provenance
  attestation, and release notes list them separately from the app installers.

### Removed
- **The two prebuilt Python utilities that shipped with the original project**
  (`IEM Curve Converter.exe`, `IEM Manifest Generator.exe`). They were unsigned
  and unauditable, added ~19 MB to every installer (they sat inside the bundled
  frontend), and are fully replaced by the Rust equivalents above. They also
  triggered antivirus heuristics: PyInstaller executables unpack an embedded
  archive at runtime, which behaviour-based engines classify as a "dropper"
  (3-4 of 70 engines flagged them, all machine-learning/heuristic detections
  consistent with a PyInstaller false positive).

## [1.1.0] - 2026-07-19 [YANKED]

> Withdrawn: the published installers bundled the upstream prebuilt `.exe`
> utilities. Superseded by 1.2.0.

DSP engine improvements: a faster real-time path and a plotting fix, with the
audio output unchanged bit-for-bit.

### Added
- Benchmark and output-fingerprint examples for the DSP core
  (`cargo run --release -p iem-core --example bench` / `--example dump`).
- `get_biquad_magnitude_legacy` (and the `biquad_magnitude_legacy` export) which
  reproduces the original plotting routine bug-for-bug, in case the old curve is
  ever needed.

### Changed
- **Plotted frequency response now matches the audio path.** The default
  magnitude function uses the corrected RBJ high-shelf; previously the plot
  disagreed with what the engine actually rendered for high-shelf filters.
- **Real-time engine is substantially faster when few bands are active.** The
  processing loop now walks only up to the highest active filter instead of
  testing all 95 filters on every sample: ~14.8x faster with nothing engaged,
  ~1.6x faster with 10 bands, and unchanged at full load. Output is bit-identical.

## [1.0.0] - 2026-07-19 [YANKED]

> Withdrawn: the published installers bundled the upstream prebuilt `.exe`
> utilities. Superseded by 1.2.0.

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

[Unreleased]: https://github.com/Londopy/IEM-Tool-rs/compare/v1.3.0...HEAD
[1.3.0]: https://github.com/Londopy/IEM-Tool-rs/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/Londopy/IEM-Tool-rs/releases/tag/v1.2.0
