/**
 * dsp-processor-wasm.js — WebAssembly-backed AudioWorkletProcessor.
 *
 * Drop-in replacement for dsp-processor.js: identical message protocol
 * (init / updatePreamp / updateFilters / updateSimulations / updateCrossover /
 * reset), but all per-sample DSP runs in the Rust `iem-core` engine compiled to
 * WASM. Verified sample-for-sample identical to the original JS worklet.
 *
 * The main thread must pass the compiled wasm bytes via processorOptions:
 *
 *   const bytes = await (await fetch('./wasm/iem_core.wasm')).arrayBuffer();
 *   await ctx.audioWorklet.addModule('./dsp-processor-wasm.js');
 *   const node = new AudioWorkletNode(ctx, 'dsp-processor-wasm', {
 *     numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [2],
 *     processorOptions: { wasmBytes: bytes, sampleRate: ctx.sampleRate },
 *   });
 *   // then post the same messages the app already sends to 'dsp-processor'.
 */
const TYPE = { peaking:0, lowshelf:1, highshelf:2, lowpass:3, highpass:4, notch:5 };
const BANK = { eq:0, sim:1, xo:2 };
const XO_TYPE = { '3way':3, '4way':4, '5way':5 };

class DspProcessorWasm extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const opts = (options && options.processorOptions) || {};
    const bytes = opts.wasmBytes;
    // Synchronous instantiate is allowed inside a worklet when bytes are provided.
    const module = new WebAssembly.Module(bytes);
    const instance = new WebAssembly.Instance(module, {});
    this.X = instance.exports;
    this.sr = opts.sampleRate || sampleRate || 44100;
    this.engine = this.X.engine_new(this.sr);

    this.cap = 0;         // capacity (frames) of the scratch buffers
    this.inL = this.inR = this.outL = this.outR = 0;
    this.ensureBuffers(128);

    this.port.onmessage = (e) => this.handle(e.data);
  }

  ensureBuffers(n) {
    if (n <= this.cap) return;
    const X = this.X;
    if (this.inL) { X.dealloc(this.inL, this.cap*4); X.dealloc(this.inR, this.cap*4); X.dealloc(this.outL, this.cap*4); X.dealloc(this.outR, this.cap*4); }
    this.inL = X.alloc(n*4); this.inR = X.alloc(n*4);
    this.outL = X.alloc(n*4); this.outR = X.alloc(n*4);
    this.cap = n;
  }

  ftype(t) { return typeof t === 'string' ? (TYPE[t] ?? 0) : t; }

  handle(data) {
    const X = this.X, eng = this.engine;
    switch (data.type) {
      case 'init':
        this.sr = data.sampleRate || this.sr;
        X.engine_set_sample_rate(eng, this.sr);
        break;
      case 'updatePreamp':
        X.engine_set_preamp(eng, Number.isFinite(data.preampDb) ? data.preampDb : 0.0);
        break;
      case 'updateFilters':
        data.filters.forEach(f =>
          X.engine_update_filter(eng, BANK.eq, f.index, this.ftype(f.filterType),
            f.frequency, f.gain, f.q, f.bypassed ? 1 : 0));
        break;
      case 'updateSimulations':
        data.sims.forEach(s =>
          X.engine_update_filter(eng, BANK.sim, s.index, this.ftype(s.filterType),
            s.frequency, s.gain, s.q, s.bypassed ? 1 : 0));
        break;
      case 'updateCrossover': {
        const g = data.gains || [1,1,1,1,1];
        X.engine_set_crossover(eng, data.enabled ? 1 : 0, XO_TYPE[data.xoType] || 3,
          g[0]??1, g[1]??1, g[2]??1, g[3]??1, g[4]??1);
        (data.filters||[]).forEach(f =>
          X.engine_update_filter(eng, BANK.xo, f.index, this.ftype(f.filterType),
            f.frequency, f.gain, f.q, f.bypassed ? 1 : 0));
        break;
      }
      case 'reset':
        X.engine_reset(eng);
        break;
    }
  }

  process(inputs, outputs) {
    const input = inputs[0];
    const output = outputs[0];
    if (!input || !input[0] || input[0].length === 0) return true;

    const inL = input[0];
    const inR = input[1] || input[0];
    const outCL = output[0];
    const outCR = output[1];
    const n = inL.length;
    this.ensureBuffers(n);

    const X = this.X, mem = X.memory.buffer;
    new Float32Array(mem, this.inL, n).set(inL);
    new Float32Array(mem, this.inR, n).set(inR);

    X.engine_process(this.engine, this.inL, this.inR, this.outL, this.outR, n);

    outCL.set(new Float32Array(mem, this.outL, n));
    if (outCR) outCR.set(new Float32Array(mem, this.outR, n));
    return true;
  }
}

registerProcessor('dsp-processor-wasm', DspProcessorWasm);
