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
    const src = ctx.createMediaElementSource(m);
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
    let node = src;
    const chain = [pre, ...eq, bass, treble];
    chain.forEach((n) => { node.connect(n); node = n; });
    node.connect(width.input); width.out.connect(feed.input); feed.out.connect(pan);
    pan.connect(spatial.input); spatial.out.connect(room.input); room.out.connect(comp); comp.connect(ctx.destination);
    g = { ctx, pre, eq, bass, treble, width, feed, depth, lfo, spatial, room, comp };
  };
  const neutral = (c) => !c.enabled || (c.preamp === 0 && c.bands.every((b) => b === 0) && c.bass === 0 && c.treble === 0 &&
    c.stereoWidth === 100 && c.crossfeed === 0 && c.reverb === "off" && c.compressor === "off" && c.spatial === "off");
  const effects = (cfg) => {
    const m = media();
    if (!m) return;
    const on = cfg.enabled;
    m.playbackRate = on ? cfg.speed / 100 : 1;
    const keep = !on || cfg.keepPitch;
    m.preservesPitch = keep; m.webkitPreservesPitch = keep; m.mozPreservesPitch = keep;
    if (!g) { if (neutral(cfg)) return; try { build(m); } catch (_) { return; } }
    const live = !neutral(cfg);
    const db = (x) => Math.pow(10, x / 20);
    g.ctx.resume().catch(() => {});
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
  // Debug spike: the audio stream address the page itself is fetching, to learn whether it can be opened elsewhere.
  let lastUrl = "";
  setInterval(() => {
    try {
      const hit = performance.getEntriesByType("resource").filter((e) => e.name.includes("videoplayback") && /mime=audio/.test(e.name)).pop();
      if (hit && hit.name !== lastUrl && id()) { lastUrl = hit.name; document.title = "LUNORX_URL|" + id() + "|" + hit.name; }
    } catch (_) {}
  }, 1000);
  window.__lunorx = {
    effects,
    play: () => { const m = media(); if (m) m.play().catch(() => {}); },
    pause: () => { const m = media(); if (m) m.pause(); },
    seek: (ms) => { const m = media(); if (m) m.currentTime = ms / 1000; },
    volume: (n) => { const m = media(); if (m) { m.volume = n / 100; m.muted = n === 0; } },
  };
})();
