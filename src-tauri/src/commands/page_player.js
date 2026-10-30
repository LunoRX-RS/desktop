// Runs inside the app-owned YouTube window, with no Tauri IPC. It reports what the page's own player is doing and
// carries out play, pause, seek and volume for the app. It makes no request of its own.
(() => {
  const P = "LUNORX_ST|";
  const id = () => new URLSearchParams(location.search).get("v") || "";
  const media = () => document.querySelector("video");
  const state = (m) => (m.ended ? "ended" : m.error ? "error" : m.paused ? "paused" : m.readyState < 3 ? "waiting" : "playing");
  let last = "";
  // The page was opened for one video. If YouTube moves on by itself (autoplay of its own "up next"), stop it: the app
  // chooses what plays next, and the track that was asked for is over.
  const first = id();
  let left = false;
  setInterval(() => {
    const m = media();
    if (!m || !id()) return;
    if (first && id() !== first) {
      m.pause();
      m.muted = true;
      if (!left) { left = true; last = ""; }
      const line = P + first + "|" + Math.round(m.currentTime * 1000) + "|0|ended";
      if (line !== last) { last = line; document.title = line; }
      return;
    }
    const dur = Number.isFinite(m.duration) ? m.duration : 0;
    const line = P + id() + "|" + Math.round(m.currentTime * 1000) + "|" + Math.round(dur * 1000) + "|" + state(m);
    if (line !== last) { last = line; document.title = line; }
  }, 500);
  // Sound shaping, the same settings the native player applies with mpv, done here with Web Audio because the sound
  // comes from this page. Built only once something is actually shaped.
  const BANDS = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
  let g = null;
  const echo = (ctx) => {
    const input = ctx.createGain(), out = ctx.createGain(), dry = ctx.createGain();
    input.connect(dry); dry.connect(out);
    const taps = [0, 1, 2].map(() => {
      const d = ctx.createDelay(1), w = ctx.createGain();
      w.gain.value = 0; input.connect(d); d.connect(w); w.connect(out);
      return { d, w };
    });
    return { input, out, dry, set: (list, mix) => {
      dry.gain.value = list.length ? 1 : 1;
      taps.forEach((t, i) => { const e = list[i]; t.w.gain.value = e ? e[1] * mix : 0; if (e) t.d.delayTime.value = e[0] / 1000; });
    } };
  };
  const matrix = (ctx) => {
    const sp = ctx.createChannelSplitter(2), mg = ctx.createChannelMerger(2);
    const mk = () => ctx.createGain();
    const ll = mk(), rl = mk(), lr = mk(), rr = mk();
    sp.connect(ll, 0); sp.connect(lr, 0); sp.connect(rl, 1); sp.connect(rr, 1);
    ll.connect(mg, 0, 0); rl.connect(mg, 0, 0); lr.connect(mg, 0, 1); rr.connect(mg, 0, 1);
    return { input: sp, out: mg, set: (same, cross) => { ll.gain.value = rr.gain.value = same; rl.gain.value = lr.gain.value = cross; } };
  };
  const build = (m) => {
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    const pre = ctx.createGain();
    const eq = BANDS.map((f) => { const b = ctx.createBiquadFilter(); b.type = "peaking"; b.frequency.value = f; b.Q.value = 1.4; return b; });
    const bass = ctx.createBiquadFilter(); bass.type = "lowshelf"; bass.frequency.value = 100;
    const treble = ctx.createBiquadFilter(); treble.type = "highshelf"; treble.frequency.value = 3000;
    const width = matrix(ctx), feed = matrix(ctx);
    const pan = ctx.createStereoPanner();
    const lfo = ctx.createOscillator(), depth = ctx.createGain();
    lfo.type = "sine"; lfo.connect(depth); depth.connect(pan.pan); depth.gain.value = 0; lfo.start();
    const spatial = echo(ctx), room = echo(ctx);
    const comp = ctx.createDynamicsCompressor();
    let node = pre;
    [...eq, bass, treble].forEach((n) => { node.connect(n); node = n; });
    node.connect(width.input); width.out.connect(feed.input); feed.out.connect(pan);
    pan.connect(spatial.input); spatial.out.connect(room.input); room.out.connect(comp);
    // DJ stage after the shaping: a sweep filter, a tempo echo and the channel level.
    const filter = ctx.createBiquadFilter(); filter.type = "allpass";
    const post = ctx.createGain(), trim = ctx.createGain();
    const delay = ctx.createDelay(4), feedback = ctx.createGain(), wet = ctx.createGain();
    feedback.gain.value = 0; wet.gain.value = 0;
    comp.connect(filter); filter.connect(post);
    post.connect(trim);
    post.connect(delay); delay.connect(feedback); feedback.connect(delay); delay.connect(wet); wet.connect(trim);
    trim.connect(ctx.destination);
    // Listens to the deck before its fader, for the waveform.
    const analyser = ctx.createAnalyser(); analyser.fftSize = 1024; analyser.smoothingTimeConstant = 0.2;
    post.connect(analyser);
    g = { ctx, pre, eq, bass, treble, width, feed, depth, lfo, spatial, room, comp, filter, trim, delay, feedback, wet, analyser, attached: false };
  };
  const neutral = (c) => !c.enabled || (c.preamp === 0 && c.bands.every((b) => b === 0) && c.bass === 0 && c.treble === 0 &&
    c.stereoWidth === 100 && c.crossfeed === 0 && c.reverb === "off" && c.compressor === "off" && c.spatial === "off");
  let baseRate = 1, bend = 0, force = false, lastCfg = null;
  const effects = (cfg) => {
    lastCfg = cfg;
    const m = media();
    if (!m) return;
    const on = cfg.enabled;
    baseRate = on ? cfg.speed / 100 : 1;
    m.playbackRate = baseRate * (1 + bend / 100);
    const keep = !on || cfg.keepPitch;
    m.preservesPitch = keep; m.webkitPreservesPitch = keep; m.mozPreservesPitch = keep;
    if (!g) { if (neutral(cfg) && !force) return; try { build(m); } catch (_) { return; } }
    const live = !neutral(cfg);
    const db = (x) => Math.pow(10, x / 20);
    // Take over the element's sound only once the graph exists and the context is really running; capturing it into a
    // suspended context would leave the page silent.
    g.ctx.resume().then(() => {
      if (g.ctx.state === "running" && !g.attached) {
        try { g.ctx.createMediaElementSource(m).connect(g.pre); g.attached = true; } catch (_) {}
      }
    }).catch(() => {});
    g.pre.gain.value = live ? db(cfg.preamp) : 1;
    g.eq.forEach((b, i) => { b.gain.value = live ? cfg.bands[i] : 0; });
    g.bass.gain.value = live ? cfg.bass : 0;
    g.treble.gain.value = live ? cfg.treble : 0;
    const four = live && cfg.spatial === "4d";
    const w = live ? (cfg.stereoWidth / 100) * (four ? 1.6 : 1) : 1;
    g.width.set((1 + w) / 2, (1 - w) / 2);
    const c = live ? cfg.crossfeed / 100 : 0;
    g.feed.set(1 - c * 0.5, c * 0.5);
    const eight = live && cfg.spatial === "8d";
    g.lfo.frequency.value = four ? 0.3 : 0.125;
    g.depth.gain.value = eight ? 1 : four ? 0.85 : 0;
    g.spatial.set(eight ? [[60, 0.2]] : four ? [[50, 0.25], [90, 0.15]] : [], 1);
    g.room.set(!live ? [] : cfg.reverb === "room" ? [[40, 0.25], [70, 0.15]] : cfg.reverb === "hall" ? [[60, 0.3], [120, 0.2], [180, 0.12]] : [], 1);
    const lv = { off: null, light: [-12, 2, 20, 250], medium: [-16, 4, 10, 200], heavy: [-22, 8, 5, 150] }[live ? cfg.compressor : "off"];
    g.comp.threshold.value = lv ? lv[0] : 0; g.comp.ratio.value = lv ? lv[1] : 1;
    g.comp.attack.value = lv ? lv[2] / 1000 : 0.003; g.comp.release.value = lv ? lv[3] / 1000 : 0.25;
  };
  // A loop the page keeps itself, so the jump back is tight rather than a round trip to the app.
  let loop = null;
  setInterval(() => {
    const m = media();
    if (loop && m && !m.paused && m.currentTime * 1000 >= loop[1]) m.currentTime = loop[0] / 1000;
  }, 25);
  // Live DJ controls. They need the audio graph, so the first one builds it even when no shaping is set.
  const ready = () => {
    if (g) return true;
    force = true;
    effects(lastCfg || { enabled: true, preamp: 0, bands: [0,0,0,0,0,0,0,0,0,0], bass: 0, treble: 0, stereoWidth: 100, crossfeed: 0, reverb: "off", compressor: "off", speed: 100, keepPitch: true, spatial: "off" });
    return !!g;
  };
  let level = { trim: 100, fader: 100 };
  const control = (c) => {
    const m = media();
    if (!m) return;
    if (c.kind === "bend") { bend = c.value; m.playbackRate = baseRate * (1 + bend / 100); return; }
    if (c.kind === "brake") {
      const from = m.playbackRate, start = performance.now();
      const step = () => {
        const k = Math.min(1, (performance.now() - start) / 1200);
        m.playbackRate = Math.max(0.07, from * (1 - k));
        if (k < 1) requestAnimationFrame(step); else { m.pause(); m.playbackRate = baseRate * (1 + bend / 100); }
      };
      requestAnimationFrame(step);
      return;
    }
    if (!ready()) return;
    if (c.kind === "filter") {
      const v = c.value;
      if (v === 0) { g.filter.type = "allpass"; return; }
      // Left of centre closes a low-pass from 20 kHz down to 200 Hz; right opens a high-pass from 20 Hz up to 8 kHz.
      g.filter.type = v < 0 ? "lowpass" : "highpass";
      const k = Math.abs(v) / 100;
      g.filter.frequency.value = v < 0 ? 20000 * Math.pow(200 / 20000, k) : 20 * Math.pow(8000 / 20, k);
      g.filter.Q.value = 0.9;
      return;
    }
    if (c.kind === "trim" || c.kind === "fader") {
      level = { ...level, [c.kind]: c.value };
      g.trim.gain.value = (level.trim / 100) * (level.fader / 100);
      return;
    }
    if (c.kind === "echo") {
      const on = c.delayMs > 0;
      if (on) g.delay.delayTime.value = Math.min(4, c.delayMs / 1000);
      g.feedback.gain.value = on ? c.feedback / 100 : 0;
      g.wet.gain.value = on ? 0.6 : 0;
    }
  };
  // Waveform: every 50 ms, the energy of the lows, mids and highs (0 to 255) at the playhead, sent in batches of four.
  let wave = [], waveAt = 0;
  const bins = new Uint8Array(512);
  setInterval(() => {
    const m = media();
    if (!m || m.paused || !id() || (first && id() !== first)) return;
    if (!g) ready();
    if (!g || !g.attached) return;
    g.analyser.getByteFrequencyData(bins);
    const hz = g.ctx.sampleRate / 1024;
    const band = (lo, hi) => {
      let sum = 0, n = 0;
      for (let i = Math.max(1, Math.floor(lo / hz)); i < Math.min(512, Math.ceil(hi / hz)); i++) { sum += bins[i]; n++; }
      return n ? Math.round(sum / n) : 0;
    };
    if (!wave.length) waveAt = Math.round(m.currentTime * 1000);
    wave.push(band(20, 250) + "." + band(250, 4000) + "." + band(4000, 16000));
    if (wave.length >= 4) { document.title = "LUNORX_WF|" + id() + "|" + waveAt + "|" + wave.join(","); wave = []; }
  }, 50);
  window.__lunorx = {
    effects,
    control,
    loop: (a, b) => { loop = Number.isFinite(a) && Number.isFinite(b) && b > a ? [a, b] : null; },
    play: () => { const m = media(); if (m) m.play().catch(() => {}); },
    pause: () => { const m = media(); if (m) m.pause(); },
    seek: (ms) => { const m = media(); if (m) m.currentTime = ms / 1000; },
    volume: (n) => { const m = media(); if (m) { m.volume = n / 100; m.muted = n === 0; } },
  };
})();
