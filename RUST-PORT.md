# IEM Tool — Rust core port

This fork moves the computational core of IEM Tool from JavaScript into **Rust**,
and lays the groundwork to replace the Electron shell with **Tauri** (Rust
backend + the existing, unchanged HTML/CSS/JS frontend).

Nothing about the original UI changes. The Rust core is verified numerically
against the original JavaScript, and the app still runs unmodified until you opt
in to the Rust paths.

---

## What was ported

Everything computational in the app now has a verified Rust implementation in
`rust/iem-core`:

| Original JS | Rust module | Notes |
|---|---|---|
| `BiquadFilter` (dsp-processor.js) — RBJ design + stereo TDF-II + param interpolation | `biquad.rs` | Real-time audio path |
| `DspProcessor.process` — preamp, 80-band EQ, 15-band sim, 3/4/5-way crossover | `engine.rs` | Full worklet engine |
| `EQ_Module.getBiquadMagnitude` | `magnitude.rs` | Frequency-response plotting/modelling |
| `CurveUtils` — log grid, `normalizeTo75dB`, `cubicSplineInterpolate`, `gaussianSmooth`, `averageCurves` | `curves.rs` | Curve maths |
| `DSP.interpolate` (log-linear) | `lib.rs` (`interp_loglinear`) | |
| `PEQDB_Module.generateLeastSquaresAutoEQ` (coordinate-descent solver) | `autoeq.rs` | AutoEQ |
| `IEM Manifest Generator.exe` | `iem-utils` (`iem-manifest-generator`) | Native Rust |
| `IEM Curve Converter.exe` | `iem-utils` (`iem-curve-converter`) | Native Rust |

The same `iem-core` crate compiles **two ways from one source**:

* `wasm32-unknown-unknown` → drives the real-time AudioWorklet and in-page
  analysis in the webview (`app-files/wasm/iem_core.wasm`).
* native `rlib` → linked into the Tauri backend and the utility crate. (The
  crate is `rlib` by default so it links cleanly into native builds and tests;
  the wasm build requests the cdylib explicitly via `cargo rustc --crate-type cdylib`,
  which the `build-wasm` scripts and CI already do.)

No external Rust crates are used by the core (no `wasm-bindgen`, no `libm`) — it
is a small, self-contained C-ABI module.

---

## Verification (Rust vs. original JS)

Ran the original JS functions and the Rust/WASM versions on the same random
inputs (`rust/` was validated in CI-style harnesses during the port):

| Check | Max error | Tolerance |
|---|---|---|
| `biquad_magnitude` (relative) | 2.5e-13 | 1e-9 |
| `cubic_spline` (abs dB) | 4.2e-4 | 5e-3 |
| `gaussian_smooth` (abs dB) | 3.5e-4 | 5e-3 |
| AutoEQ gains (abs dB) | 1.2e-7 | 2e-2 |
| AutoEQ preamp (abs dB) | 2.0e-7 | 2e-2 |
| **Engine audio output** (per-sample) | **0.0 (exact)** | 1e-6 |

The small spline/smoothing differences are because the original JS used
`Float32Array` internally while Rust uses `f64` — i.e. Rust is *more* accurate,
not different.

The **manifest generator** was validated by regenerating `manifest.json` from
the full 10,615-file library: the file list is identical, and sizes match the
committed manifest byte-for-byte on Windows (CRLF).

### One bug found during the port — now fixed

The JS plotting routine `getBiquadMagnitude` computes the **high-shelf `a1`**
coefficient as `2*((A-1) + (A+1)*cosW0)`, but the RBJ cookbook and the actual
audio path (`calculateCoefficients`) use `-`. So the *plotted* high-shelf curve
didn't match what you *heard*.

The port initially reproduced this faithfully for plot parity. **As of the
current version the corrected form is the default**: `get_biquad_magnitude` now
uses the RBJ sign, so the graph agrees with the engine. A test evaluates the
engine's own designed coefficients on the unit circle and asserts the plotted
magnitude matches to 1e-9, so the two can't silently diverge again.

The original curve is still available as `get_biquad_magnitude_legacy` (exported
to WASM as `biquad_magnitude_legacy`, and as `IEMCore.biquadMagnitudeLegacy()`
in the JS bridge) if you ever need to reproduce the old plot exactly.

---

## Performance

The core ships a zero-dependency benchmark and an output-fingerprint tool:

```powershell
cd E:\Python\IEM-Tool\rust
cargo run --release -p iem-core --example bench   # ns/frame + realtime factor
cargo run --release -p iem-core --example dump    # FNV fingerprint of engine output
```

`dump` exists so refactors can be proven **bit-exact**: capture the fingerprints
before a change and diff them after.

The real-time loop originally tested the bypass flag of all 95 filters on every
sample. It now walks only up to the highest active filter, so unused bands cost
nothing. Measured on 128-frame blocks at 48 kHz:

| Active filters | Before | After | Change |
|---|---|---|---|
| none (bypassed) | 31.1 ns/frame | **2.1 ns/frame** | **14.8x faster** |
| 10 EQ bands | 72.9 ns/frame | **45.4 ns/frame** | **1.6x faster** |
| 32 EQ bands | 143.5 ns/frame | 124.8 ns/frame | 1.15x faster |
| 80 EQ (max) | 311.5 ns/frame | 304.7 ns/frame | parity |
| 80 EQ + 15 sim + crossover | 378.2 ns/frame | 384.5 ns/frame | parity |

Output is bit-identical before and after — verified with `dump` across all five
configurations. Even at full load the engine runs ~48x faster than real time.

> A filter-major variant (each filter processing the whole block) was tried
> first: 9x faster when idle but ~36% *slower* at full load, because it loses
> register residency and makes 80 passes over the block in memory. Sample-major
> with a high-water mark won on both ends.

---

## Repository layout

```
IEM-Tool-rs/
├── app-files/                     # unchanged frontend + Rust glue
│   ├── index.html                 # 2 small, reversible edits (see below)
│   ├── data/                      # measurement library (~10,600 curves)
│   ├── dsp-processor.js           # original JS worklet (kept as fallback)
│   ├── dsp-processor-wasm.js      # WASM-backed worklet (drop-in)
│   ├── iem-core.js                # main-thread WASM bridge (window.IEMCore)
│   └── wasm/iem_core.wasm         # compiled Rust core (~64 KB)
├── rust/
│   ├── Cargo.toml                 # workspace
│   ├── iem-core/                  # the DSP core (native + wasm)
│   │   ├── src/                   # biquad, engine, magnitude, curves, autoeq
│   │   ├── tests/                 # core test suite
│   │   └── examples/              # bench.rs (perf), dump.rs (bit-exactness)
│   ├── iem-utils/                 # manifest generator, curve converter, EQ exporter
│   ├── src-tauri/                 # Tauri backend (commands wrap iem-core/-utils)
│   └── build-wasm.sh / .ps1       # rebuild wasm into app-files/wasm/
├── tools/                         # build.ps1, Create-Shortcut.ps1
├── install.ps1                    # optional checksum-verified installer
├── .github/workflows/             # ci.yml, release.yml
├── main.js, package.json          # original Electron shell (still works)
├── CHANGELOG.md  CREDITS.md  LICENSE  README-original.md
└── RUST-PORT.md                   # this file
```

---

## Finishing the local fork

The measurement database was delivered as a single `app-files/data.zip`
(10,615 files transfer far faster as one archive). Extract it once:

```powershell
cd E:\Python\IEM-Tool\app-files
Expand-Archive -Path data.zip -DestinationPath . -Force   # creates data\
```

This ships as a **standalone repository** (`IEM-Tool-rs`), not a GitHub fork, so
the substantial Rust/Tauri work stands on its own — while `README.md`, `LICENSE`,
and `CREDITS.md` prominently credit the original author for the reused frontend.

Create an empty `IEM-Tool-rs` repo on GitHub (do **not** click "Fork"), then:

```powershell
cd E:\Python\IEM-Tool
git init
git add .
git commit -m "IEM-Tool-rs: Rust core + Tauri shell, based on IEM Tool by MyLittlePrimordia (MIT)"
git branch -M main
git remote add origin https://github.com/<you>/IEM-Tool-rs.git
git push -u origin main
```

Attribution checklist before pushing: keep `LICENSE` (both copyright lines),
`CREDITS.md`, `README-original.md`, and the credit banner at the top of
`README.md` intact; fill your GitHub username into the `LICENSE` copyright line.

---

## Building

### 1. The WebAssembly core (works today, in Electron or Tauri)

```powershell
# one-time
rustup target add wasm32-unknown-unknown
# build + copy into app-files\wasm\
cd E:\Python\IEM-Tool\rust
.\build-wasm.ps1
```

Then run the existing app (`npm start`). To use the **Rust audio engine**, set
a flag before audio starts — e.g. in the browser console or near the top of the
app script:

```js
window.IEM_USE_RUST_DSP = true;   // use dsp-processor-wasm.js (Rust) for audio
```

Leave it unset and everything behaves exactly as before. `window.IEMCore` is
always available (`await IEMCore.ready`) for routing analysis calls
(`IEMCore.biquadMagnitude`, `.cubicSpline`, `.gaussianSmooth`,
`.normalizeTo75dB`, `.autoeqSolve`, …) into Rust.

### 2. The Tauri app (majority-Rust shell, keeps the frontend)

Prerequisites (Windows): Rust, and the
[Tauri v2 prerequisites](https://tauri.app/start/prerequisites/) (WebView2 is
already on Windows 10/11; install the Tauri CLI).

```powershell
cargo install tauri-cli --version "^2"
cd E:\Python\IEM-Tool\rust
cargo tauri dev      # run
cargo tauri build    # produce installers in rust\target\release\bundle\
```

`src-tauri/tauri.conf.json` serves `app-files/` directly as the frontend — no
bundler, no build step. The backend (`src-tauri/src/lib.rs`) exposes the core as
commands you can call from the frontend with `invoke(...)`:

```js
import { invoke } from '@tauri-apps/api/core';
const mag   = await invoke('biquad_magnitude', { ftype: 0, f, f0, q, g, fs });
const curve = await invoke('cubic_spline', { points, targets });
const eq    = await invoke('autoeq_solve', { targetCorrection, freqs, bandFreqs, bandQs, fs });
await invoke('generate_manifest', { root: '<app-files path>' });
```

### 3. The utility binaries

```powershell
cd E:\Python\IEM-Tool\rust
cargo build --release -p iem-utils
# rust\target\release\iem-manifest-generator.exe   (replaces the manifest .exe)
# rust\target\release\iem-curve-converter.exe      (replaces the converter .exe)
# rust\target\release\iem-autoeq-to-graphiceq.exe  (new: ParametricEQ -> GraphicEQ)
```

`iem-autoeq-to-graphiceq` converts a standard ParametricEQ file (AutoEq
`Preamp:` + `Filter N: ON PK ...` format) into a GraphicEQ correction curve by
evaluating the combined biquad response through `iem-core` (using the corrected
RBJ high-shelf so shelves are accurate). A generic tool for any equalizer app
that consumes the GraphicEQ format:

```powershell
iem-autoeq-to-graphiceq input.txt            # "GraphicEQ: ..." one-liner
iem-autoeq-to-graphiceq input.txt --pairs -o out.txt   # "freq gain" lines
# flags: --points N  --fs HZ  --clamp DB  --no-normalize  --preamp DB
```

---

## Status / suggested next steps

**Done & verified:** the entire computational core in Rust (native + WASM), both
utility tools, the Tauri workspace, the WASM audio worklet, the main-thread
bridge, and non-breaking hooks in `index.html`.

**Iterative (needs the running GUI to validate):**

1. Route the in-page analysis calls (`EQ_Module.getBiquadMagnitude`,
   `CurveUtils.*`, the AutoEQ button) through `window.IEMCore` — the bridge is
   ready; it's a matter of swapping call sites and A/B testing visually.
2. Move audio playback fully native (Rust `cpal`) instead of WASM-in-webview, if
   you want the audio thread out of the browser entirely.
3. Decide plot vs. audio high-shelf consistency (see the bug note above).

The `index.html` edits are just: one `<script src="iem-core.js">` include, and a
flag-gated branch in `ensureDSPGraph` that picks the WASM worklet when
`window.IEM_USE_RUST_DSP === true`. Both are inert by default.

---

## Continuous integration & releases (`.github/workflows/`)

The original repo had a single Electron build for macOS + Linux, no tests, and no
checksums. This fork replaces that with two workflows:

### `ci.yml` — on every push / PR
* **`test`** job runs on **Ubuntu, macOS and Windows**: `cargo fmt --check`,
  `cargo clippy`, the full Rust test suite (`cargo test -p iem-core -p iem-utils`,
  30 tests), a `wasm32` build, and a check that the WASM module exports the
  expected symbols.
* **`utilities`** job smoke-tests the utility binaries end-to-end (manifest
  generator round-trips a tiny library; curve converter averages an L/R pair).

### `release.yml` — on a `v*` tag (or manual dispatch)
* Builds the **Tauri** app on a matrix of **Windows (64- and 32-bit),
  macOS Apple Silicon (`macos-14`) and Linux**, extracting `data.zip` first so
  the measurement library is bundled. (Intel macOS is intentionally not built —
  it roughly doubled the release time for a shrinking share of Macs. Add a
  `macos-13` / `x86_64-apple-darwin` matrix row back if you ever want it.)
* A final `release` job downloads every installer, computes **SHA-256
  checksums**, and publishes a GitHub Release whose description embeds a
  `SHA256SUMS.txt` table (verify with `sha256sum -c SHA256SUMS.txt`). The
  checksum file is also attached as a release asset.

Cut a release with:

```powershell
git tag v1.0.1
git push origin v1.0.1
```

### Test suite

30 Rust tests cover: biquad transparency at 0 dB, center-frequency gain,
low-pass roll-off, coefficient finiteness, the faithful-vs-RBJ high-shelf
variants, spline knot interpolation, Gaussian-smoothing of a constant, log-grid
endpoints, normalization alignment, curve averaging stability, AutoEQ zero/
single-band recovery, engine bypass = pre-amp-only, and engine determinism;
the utility tests (measurement parsing, channel detection, name standardization,
L/R averaging, manifest scan/sort/size, JSON shape); and the ParametricEQ→GraphicEQ
exporter tests (filter parsing, physically-correct low/high shelves, peak
normalization, clamping, and output formatting).

> While writing these tests, a second latent bug surfaced: the original
> `cubicSplineInterpolate` reads past its coefficient arrays (returning `NaN`)
> when a target frequency exactly equals an interior knot. The app never hit it
> because its fixed 500-point grid never lands exactly on a knot, but the Rust
> version now handles it correctly (returns the knot value) instead of trapping.

---

## Release artifacts & architectures

`release.yml` builds a 5-target matrix:

| Platform | Arch | Installers |
|---|---|---|
| Windows | x86_64 (64-bit) | `-setup.exe` (NSIS), `.msi` |
| Windows | i686 (**32-bit**) | `-setup.exe` (NSIS), `.msi` |
| macOS | aarch64 (Apple Silicon) | `.dmg`, `.app.tar.gz` |
| Linux | x86_64 | `.AppImage`, `.deb` |

Every artifact gets a SHA-256 line in the release notes + `SHA256SUMS.txt`, plus a
signed **build-provenance attestation** (verify with `gh attestation verify <file> -R <owner>/<repo>`).

Notes on other arches: **Windows ARM64** (`aarch64-pc-windows-msvc`) and **Linux
ARM64** can be added as extra matrix rows if wanted; **macOS is 64-bit only**
(Apple dropped 32-bit); **32-bit Linux** is impractical (webkit2gtk 32-bit isn't
maintained). 32-bit Windows uses `--target i686-pc-windows-msvc`.

## Licensing & attribution

The original IEM Tool is MIT-licensed (declared in its `package.json`). This
fork keeps the entire frontend, so it is a derivative work: `LICENSE` retains
MyLittlePrimordia's copyright for the original and adds yours for the Rust port,
and `CREDITS.md` spells out who did what. Keep both intact when redistributing.
Fill your name into the `<your name here>` line in `LICENSE`.

## Repo hygiene added

`.gitignore` (Rust `target/`, `node_modules/`, Tauri `gen/`), `.gitattributes`
(pins `data/**` and `manifest.json` to byte-stable line endings so manifest
sizes stay deterministic across OSes), and `.github/dependabot.yml` (weekly
updates for Actions + Cargo deps).
