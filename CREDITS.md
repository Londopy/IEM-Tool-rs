# Credits & attribution

`IEM-Tool-rs` is a Rust/Tauri rewrite of the computational core of **IEM Tool**,
built directly on top of that project's frontend, UI, and data. This file records
exactly what originates where, so credit is unambiguous.

Everything here is MIT-licensed (see [`LICENSE`](LICENSE)). Two copyright holders:
**MyLittlePrimordia** (original work) and **Londopy** (this port).

---

## Original work — © MyLittlePrimordia

Source: <https://github.com/MyLittlePrimordia/IEM-Tool>

The original author designed and built the application. This port reuses that
work essentially unchanged, and it remains the larger part of the codebase:

- **The entire user interface** — `app-files/index.html` (~25k lines), all layout,
  styling, interactions, and application logic.
- **App design & UX** — the review dashboard, EQ engine UI, audio test lab,
  visualizer views, settings, and every feature (Rate & Review, 10-band EQ,
  AutoEQ, Find Similar, auto-tagging, 3D surround, blind A/B test, hearing test,
  burn-in timer, score-card export, presets, save & compare, offline operation).
- **Review-card themes** — Void, Graphite, Midnight, Ocean, Crimson, Sakura,
  Sunset, Synthwave, Pip-Boy (`app-files/themes/`).
- **Visual effects & visualizers** — `app-files/effects/` (liquid_fiber,
  neon_stars, cosmic_vortex, …) and the in-app music visualizers.
- **The measurement-curve library** — the ~10,600 IEM/headphone frequency-response
  files under `app-files/data/` (shipped here as `app-files/data.zip`), plus
  `manifest.json`.
- **The DSP algorithms themselves** — the RBJ biquad designs, the interpolating
  filter, the frequency-response evaluation, the curve maths (normalization,
  cubic-spline, Gaussian smoothing, averaging), and the coordinate-descent
  AutoEQ solver were all **authored in JavaScript by the original project**
  (`dsp-processor.js`, `EQ_Module`, `CurveUtils`, `PEQDB_Module`). The Rust code
  below is a faithful **translation** of that logic — the mathematics and design
  are the original author's.
- **Original assets** — icons (`icon.ico/.png/.icns`), fonts (`app-files/fonts/`),
  the original Electron shell (`main.js`), and the original README
  (preserved verbatim as [`README-original.md`](README-original.md)).
- The concepts for the two helper tools (**IEM Manifest Generator**,
  **IEM Curve Converter**), originally distributed as Python-built `.exe`s.

---

## This port — © Londopy

New engineering added on top of the original. Where these translate original
algorithms, correctness was verified against the original JavaScript (see
[`RUST-PORT.md`](RUST-PORT.md) for the parity numbers).

### `rust/iem-core` — DSP & analysis core (Rust → native + WebAssembly)
| File | Ported from (original JS) |
|---|---|
| `biquad.rs` | `BiquadFilter` in `dsp-processor.js` (RBJ design, stereo TDF-II, param interpolation) |
| `engine.rs` | `DspProcessor.process` (pre-amp, 80-band EQ, 15-band sim, 3/4/5-way crossover) |
| `magnitude.rs` | `EQ_Module.getBiquadMagnitude` (frequency-response evaluation) |
| `curves.rs` | `CurveUtils` (log grid, `normalizeTo75dB`, cubic spline, Gaussian smooth, averaging) |
| `autoeq.rs` | `PEQDB_Module.generateLeastSquaresAutoEQ` (coordinate-descent solver) |
| `lib.rs` | C-ABI + `DSP.interpolate` (log-linear); no external crates |

### `rust/iem-utils` — helper tools rewritten as native Rust
- `iem-manifest-generator` — reimplements the Manifest Generator; verified to
  reproduce the committed `manifest.json` for the full ~10,600-file library.
- `iem-curve-converter` — reimplements the Curve Converter (raw `.txt`/`.csv`
  parsing, L/R averaging, name standardization). Reconstructed from the tool's
  documented behavior (the original was a compiled binary, not source).
- `iem-autoeq-to-graphiceq` — new: converts a standard ParametricEQ file
  (AutoEq `Preamp:` + `Filter N: ON PK …` format) into a GraphicEQ correction
  curve by evaluating the combined biquad response through `iem-core`. A generic
  interoperability tool for any equalizer app that consumes the GraphicEQ format.

### `rust/src-tauri` — desktop shell
- A **Tauri v2** (Rust) shell replacing the original Electron shell; serves the
  unchanged `app-files/` frontend and exposes the core via `invoke` commands.

### Browser integration (`app-files/`)
- `wasm/iem_core.wasm` — the compiled Rust core.
- `iem-core.js` — main-thread WASM bridge (`window.IEMCore`).
- `dsp-processor-wasm.js` — WASM-backed AudioWorklet (drop-in for `dsp-processor.js`).
- Two small, flag-gated, non-breaking hooks in `index.html`.

### Quality & delivery
- **Test suite** — 20 Rust tests (`rust/iem-core/tests/`, `rust/iem-utils/tests/`).
- **CI** (`.github/workflows/ci.yml`) — fmt, clippy, tests, WASM build on
  Windows/macOS/Linux, plus utility smoke tests.
- **Releases** (`.github/workflows/release.yml`) — Tauri builds for Windows
  (64- & 32-bit), macOS (Apple Silicon) and Linux, with SHA-256
  checksums and build-provenance attestations.
- Repo hygiene: `.gitignore`, `.gitattributes`, `dependabot.yml`.
- Two latent bugs found and documented during the port (a high-shelf coefficient
  sign in the plot path, and an out-of-range spline read) — see `RUST-PORT.md`.

---

## Third-party components

- **Frontend libraries bundled by the original app** — Tailwind CSS
  (`app-files/js/tailwindcss.js`) and Chart.js (`app-files/js/chart.js`),
  under their respective (MIT) licenses.
- **Runtime/build stack for this port** — the Rust toolchain, the
  [Tauri](https://tauri.app) framework and its ecosystem crates
  (`serde`, `serde_json`, `tauri-plugin-dialog`), each under their own licenses.

---

If you redistribute this project in any form, keep the attribution to
**MyLittlePrimordia** ([original project](https://github.com/MyLittlePrimordia/IEM-Tool))
intact, along with this file and `LICENSE`.
