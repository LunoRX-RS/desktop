// Runs inside the app-owned YouTube window, with no Tauri IPC. It reports what the page's own player is doing and
// carries out play, pause, seek and volume for the app. It makes no request of its own.
(() => {
  const P = "LUNORX_ST|";
  const id = () => new URLSearchParams(location.search).get("v") || "";
  const media = () => document.querySelector("video");
  const state = (m) => (m.ended ? "ended" : m.error ? "error" : m.paused ? "paused" : m.readyState < 3 ? "waiting" : "playing");
  let last = "";
  setInterval(() => {
    const m = media();
    if (!m || !id()) return;
    const dur = Number.isFinite(m.duration) ? m.duration : 0;
    const line = P + id() + "|" + Math.round(m.currentTime * 1000) + "|" + Math.round(dur * 1000) + "|" + state(m);
    if (line !== last) { last = line; document.title = line; }
  }, 500);
  window.__lunorx = {
    play: () => { const m = media(); if (m) m.play().catch(() => {}); },
    pause: () => { const m = media(); if (m) m.pause(); },
    seek: (ms) => { const m = media(); if (m) m.currentTime = ms / 1000; },
    volume: (n) => { const m = media(); if (m) { m.volume = n / 100; m.muted = n === 0; } },
  };
})();
