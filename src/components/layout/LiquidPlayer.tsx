"use client";

import { useEffect, useState, type ReactNode } from "react";
import LiquidGlass from "liquid-glass-react";

// The player bar behind Apple-style liquid glass. The refraction needs an SVG filter, which Safari (and so the macOS
// webview) only partly supports, so there it shows the frost and tint without the bending. Reduced transparency gets the
// plain bar, as the rest of the app does.
export function LiquidPlayer({ children }: { children: ReactNode }) {
  const [plain, setPlain] = useState(false);
  useEffect(() => {
    const q = window.matchMedia("(prefers-reduced-transparency: reduce)");
    const sync = () => setPlain(q.matches);
    sync();
    q.addEventListener("change", sync);
    return () => q.removeEventListener("change", sync);
  }, []);
  if (plain) return <>{children}</>;
  return (
    <LiquidGlass cornerRadius={24} displacementScale={48} blurAmount={0.08} saturation={140} aberrationIntensity={1.5} elasticity={0.12} className="w-full" style={{ width: "100%" }}>
      {children}
    </LiquidGlass>
  );
}
