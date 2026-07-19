# 🎧 IEM-Tool-rs

**A Rust rewrite of [IEM Tool](https://github.com/MyLittlePrimordia/IEM-Tool) — Parametric EQ & Frequency-Response Analyzer for in-ear monitors.**

> ### 🙏 Credit where it's due
> This project is built on **[IEM Tool](https://github.com/MyLittlePrimordia/IEM-Tool) by [MyLittlePrimordia](https://github.com/MyLittlePrimordia)**.
> The **entire user interface** — the HTML/CSS/JS frontend, the app design, the
> review-card themes, the visualizers, and the measurement-curve library — is
> their original work, used here under the MIT License. All credit for the app's
> look, feel, and features belongs to them. Please ⭐ [the original project](https://github.com/MyLittlePrimordia/IEM-Tool).
>
> **What this repo adds:** the computational core, DSP engine, utilities, and
> desktop shell rewritten in **Rust**, plus cross-platform CI and releases.
> See **[CREDITS.md](CREDITS.md)** for a precise breakdown of who did what.

---

## What's different from the original

| | Original IEM Tool | IEM-Tool-rs |
|---|---|---|
| DSP / EQ core | JavaScript | **Rust** (native + WebAssembly) |
| Manifest generator & curve converter | Python `.exe` | **Rust** binaries |
| Parametric-to-GraphicEQ export | — | new **Rust** tool |
| Desktop shell | Electron | **Tauri** (Rust) — smaller, faster, no bundled Chromium |
| Tests | none | **29 Rust tests** in CI |
| CI builds | macOS + Linux (Electron) | **Windows (64 + 32-bit), macOS (Intel + Apple Silicon), Linux** |
| Release checksums | none | **SHA-256 + build provenance attestations** |

The frontend and all its features (Rate & Review, 10-band EQ, AutoEQ, Find
Similar, 3D surround, blind A/B test, hearing test, burn-in timer, visualizers,
themes, 100% offline, …) are unchanged — see **[README-original.md](README-original.md)**
for the original author's full feature write-up and screenshots.

---

## Quick start

```powershell
# 1. Extract the measurement database (shipped as a single archive)
cd app-files
Expand-Archive -Path data.zip -DestinationPath . -Force

# 2. Build & run the Tauri (Rust) desktop app
cd ..\rust
cargo install tauri-cli --version "^2"
cargo tauri dev        # run
cargo tauri build      # build installers
```

Full build / architecture / CI details are in **[RUST-PORT.md](RUST-PORT.md)**.

The original Electron shell (`main.js` + `npm start`) still works too, and the
Rust DSP core can be enabled inside it with `window.IEM_USE_RUST_DSP = true`.

---

## Platforms

Release builds are produced by GitHub Actions for:

- **Windows** — 64-bit (`x86_64`) and 32-bit (`i686`) — NSIS `.exe` + `.msi`
- **macOS** — Intel (`x86_64`) and Apple Silicon (`aarch64`) — `.dmg`
- **Linux** — `x86_64` — `.AppImage` + `.deb`

Each artifact ships with a SHA-256 checksum and a signed build-provenance
attestation.

---

## Optional: install script & shortcuts

The normal way to install is to grab the `-setup.exe` from
[Releases](https://github.com/Londopy/IEM-Tool-rs/releases). These are optional
extras, not requirements:

**`install.ps1`** — downloads the right installer for your architecture,
**verifies its SHA-256** against the release's `SHA256SUMS.txt`, then runs it:

```powershell
.\install.ps1
# or, one-liner:
irm https://raw.githubusercontent.com/Londopy/IEM-Tool-rs/main/install.ps1 | iex
# options: -Arch x86 | -Tag v1.0.0 | -DownloadOnly
```

**`tools\build.ps1`** — build from source (needs Rust + the
[Tauri prerequisites](https://tauri.app/start/prerequisites/)):

```powershell
.\tools\build.ps1          # release build
.\tools\build.ps1 -Dev     # hot-reload dev window
.\tools\build.ps1 -Wasm    # also rebuild the WebAssembly core
```

**`tools\Create-Shortcut.ps1`** — makes desktop `.lnk` shortcuts for the above:

```powershell
.\tools\Create-Shortcut.ps1 -For both
```

> The shortcut is **generated on your machine** rather than shipped in the
> release on purpose: a `.lnk` downloaded from the internet that launches
> PowerShell matches a known malware pattern and gets blocked by Defender /
> SmartScreen. Generating it locally carries no Mark-of-the-Web, so it just works.

---

## License & attribution

Released under the **MIT License** (see [LICENSE](LICENSE)), preserving the
original author's copyright for the reused frontend and adding the author of this
port for the Rust/Tauri work. If you redistribute, keep the attribution to
[MyLittlePrimordia](https://github.com/MyLittlePrimordia/IEM-Tool) intact.
