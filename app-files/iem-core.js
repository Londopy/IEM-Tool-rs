/**
 * iem-core.js — main-thread bridge to the Rust core compiled to WebAssembly.
 *
 * Loads app-files/wasm/iem_core.wasm and exposes the ported computational core
 * (biquad magnitude, curve utilities, AutoEQ) to the existing frontend. Every
 * function is verified to match the original JS to <1e-3 (magnitude/AutoEQ to
 * ~1e-13). Designed so the app can route getBiquadMagnitude / CurveUtils /
 * AutoEQ through Rust, with the original JS kept as a fallback.
 *
 * Usage:
 *   await IEMCore.ready;                       // resolve once before using
 *   IEMCore.biquadMagnitude('peaking', f, f0, q, g, fs);
 */
const IEMCore = (() => {
  const TYPE = { peaking:0, lowshelf:1, highshelf:2, lowpass:3, highpass:4, notch:5 };
  let X = null; // wasm exports

  // ---- linear-memory helpers (re-read buffer each call; it can grow) --------
  function f64in(arr) {
    const p = X.alloc(arr.length * 8);
    new Float64Array(X.memory.buffer, p, arr.length).set(arr);
    return p;
  }
  function f64out(n) { return X.alloc(n * 8); }
  function readF64(p, n) { return new Float64Array(X.memory.buffer, p, n).slice(); }
  function free(p, n) { X.dealloc(p, n * 8); }

  async function init(url) {
    const res = await fetch(url || './wasm/iem_core.wasm', { cache: 'no-store' });
    const bytes = await res.arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {});
    X = instance.exports;
    return API;
  }

  const API = {
    TYPE,
    get exports() { return X; },

    /** Magnitude using the corrected RBJ high-shelf (matches the audio engine). */
    biquadMagnitude(type, f, f0, q, g, fs) {
      const t = typeof type === 'string' ? (TYPE[type] ?? 0) : type;
      return X.biquad_magnitude(t, f, f0, q, g, fs);
    },

    /** The original routine's curve, including its high-shelf sign quirk. */
    biquadMagnitudeLegacy(type, f, f0, q, g, fs) {
      const t = typeof type === 'string' ? (TYPE[type] ?? 0) : type;
      return X.biquad_magnitude_legacy(t, f, f0, q, g, fs);
    },

    /** bands: [{type,f0,q,g}], returns combined dB per target freq. */
    chainMagnitudeDb(bands, freqs, fs) {
      const flat = new Float64Array(bands.length * 4);
      bands.forEach((b, i) => {
        flat[4*i]   = typeof b.type === 'string' ? (TYPE[b.type] ?? 0) : b.type;
        flat[4*i+1] = b.f0; flat[4*i+2] = b.q; flat[4*i+3] = b.g;
      });
      const bp = f64in(flat), fp = f64in(freqs), op = f64out(freqs.length);
      X.chain_magnitude_db(bp, bands.length, fp, freqs.length, op, fs);
      const out = readF64(op, freqs.length);
      free(bp, flat.length); free(fp, freqs.length); free(op, freqs.length);
      return out;
    },

    /** points: [[hz,db],...]; targets: [hz,...] */
    cubicSpline(points, targets) {
      const flat = new Float64Array(points.length * 2);
      points.forEach((p, i) => { flat[2*i] = p[0]; flat[2*i+1] = p[1]; });
      const pp = f64in(flat), tp = f64in(targets), op = f64out(targets.length);
      X.cubic_spline(pp, points.length, tp, targets.length, op);
      const out = readF64(op, targets.length);
      free(pp, flat.length); free(tp, targets.length); free(op, targets.length);
      return out;
    },

    interpLogLinear(points, targets) {
      const flat = new Float64Array(points.length * 2);
      points.forEach((p, i) => { flat[2*i] = p[0]; flat[2*i+1] = p[1]; });
      const pp = f64in(flat), tp = f64in(targets), op = f64out(targets.length);
      X.interp_loglinear(pp, points.length, tp, targets.length, op);
      const out = readF64(op, targets.length);
      free(pp, flat.length); free(tp, targets.length); free(op, targets.length);
      return out;
    },

    gaussianSmooth(freqs, values, octaveBw = 0.08) {
      const fp = f64in(freqs), vp = f64in(values), op = f64out(freqs.length);
      X.gaussian_smooth(fp, vp, freqs.length, octaveBw, op);
      const out = readF64(op, freqs.length);
      free(fp, freqs.length); free(vp, freqs.length); free(op, freqs.length);
      return out;
    },

    /** data: [[hz,db],...]; modeHz: number or null/'mean'. returns [[hz,db],...] */
    normalizeTo75dB(data, modeHz = null, targetDb = 75) {
      const flat = new Float64Array(data.length * 2);
      data.forEach((p, i) => { flat[2*i] = p[0]; flat[2*i+1] = p[1]; });
      const mode = (modeHz == null || modeHz === 'mean') ? NaN : Number(modeHz);
      const dp = f64in(flat), op = f64out(data.length * 2);
      X.normalize_to_75db(dp, data.length, mode, targetDb, op);
      const raw = readF64(op, data.length * 2);
      free(dp, flat.length); free(op, data.length * 2);
      const out = new Array(data.length);
      for (let i = 0; i < data.length; i++) out[i] = [raw[2*i], raw[2*i+1]];
      return out;
    },

    logGrid(numPoints = 500) {
      const op = f64out(numPoints);
      X.generate_log_grid(numPoints, op);
      const out = readF64(op, numPoints);
      free(op, numPoints);
      return out;
    },

    /** returns { gains:[...], preamp } */
    autoeqSolve(targetCorrection, freqs, bandFreqs, bandQs, fs = 44100) {
      const tp = f64in(targetCorrection), fp = f64in(freqs);
      const bfp = f64in(bandFreqs), bqp = f64in(bandQs), gp = f64out(bandFreqs.length);
      const preamp = X.autoeq_solve(tp, fp, freqs.length, bfp, bqp, bandFreqs.length, fs, gp);
      const gains = readF64(gp, bandFreqs.length);
      free(tp, freqs.length); free(fp, freqs.length);
      free(bfp, bandFreqs.length); free(bqp, bandFreqs.length); free(gp, bandFreqs.length);
      return { gains: Array.from(gains), preamp };
    },
  };

  API.ready = init();
  return API;
})();

if (typeof window !== 'undefined') window.IEMCore = IEMCore;
if (typeof module !== 'undefined') module.exports = IEMCore;
