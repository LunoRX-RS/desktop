"use client";

import { useRef } from "react";

/**
 * A rotary knob like a DJ mixer's. Drag up or down (Shift for fine), scroll, or use the arrow keys; double-click resets.
 * It is a slider to assistive technology.
 */
export function Knob({ label, min, max, value, reset, show, onChange, size = 52 }: {
  label: string; min: number; max: number; value: number; reset: number; show: (v: number) => string; onChange: (v: number) => void; size?: number;
}) {
  const drag = useRef<{ y: number; v: number } | null>(null);
  const clamp = (v: number) => Math.round(Math.min(max, Math.max(min, v)));
  const span = max - min;
  const k = span > 0 ? (value - min) / span : 0;
  // 270 degrees of travel, gap at the bottom.
  const angle = -135 + k * 270;
  const r = 20, c = 26;
  const arc = (from: number, to: number) => {
    const p = (a: number) => [c + r * Math.sin((a * Math.PI) / 180), c - r * Math.cos((a * Math.PI) / 180)];
    const [x1, y1] = p(from), [x2, y2] = p(to);
    return `M ${x1} ${y1} A ${r} ${r} 0 ${Math.abs(to - from) > 180 ? 1 : 0} ${to > from ? 1 : 0} ${x2} ${y2}`;
  };
  const centre = -135 + ((reset - min) / (span || 1)) * 270;
  const moved = value !== reset;
  return (
    <div className="flex flex-col items-center gap-1">
      <div
        role="slider"
        tabIndex={0}
        aria-label={label}
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={value}
        aria-valuetext={show(value)}
        onPointerDown={(e) => { e.currentTarget.setPointerCapture(e.pointerId); drag.current = { y: e.clientY, v: value }; }}
        onPointerMove={(e) => { const d = drag.current; if (!d) return; const per = (e.shiftKey ? 600 : 160) / span; onChange(clamp(d.v + (d.y - e.clientY) / per)); }}
        onPointerUp={() => { drag.current = null; }}
        onPointerCancel={() => { drag.current = null; }}
        onDoubleClick={() => onChange(reset)}
        onWheel={(e) => onChange(clamp(value + (e.deltaY < 0 ? 1 : -1) * Math.max(1, Math.round(span / 50))))}
        onKeyDown={(e) => {
          const step = Math.max(1, Math.round(span / (e.shiftKey ? 100 : 25)));
          if (e.key === "ArrowUp" || e.key === "ArrowRight") { e.preventDefault(); onChange(clamp(value + step)); }
          else if (e.key === "ArrowDown" || e.key === "ArrowLeft") { e.preventDefault(); onChange(clamp(value - step)); }
          else if (e.key === "Home") onChange(min);
          else if (e.key === "End") onChange(max);
          else if (e.key === "Backspace" || e.key === "Delete") onChange(reset);
        }}
        className="cursor-ns-resize touch-none rounded-full focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-text"
      >
        <svg viewBox="0 0 52 52" width={size} height={size} aria-hidden>
          <path d={arc(-135, 135)} fill="none" strokeWidth="3.5" strokeLinecap="round" className="stroke-frost-300" />
          {moved && <path d={arc(Math.min(centre, angle), Math.max(centre, angle))} fill="none" strokeWidth="3.5" strokeLinecap="round" className="stroke-primary-text" />}
          <circle cx={c} cy={c} r="14" className="fill-frost-200" />
          <line x1={c} y1={c} x2={c + 11 * Math.sin((angle * Math.PI) / 180)} y2={c - 11 * Math.cos((angle * Math.PI) / 180)} strokeWidth="2.5" strokeLinecap="round" className="stroke-primary-text" />
        </svg>
      </div>
      <span className="text-[11px] font-medium text-secondary-text">{label}</span>
      <span className="text-[10px] tabular-nums text-tertiary-text">{show(value)}</span>
    </div>
  );
}

/** A vertical channel fader. Drag the cap, use the arrows, or double-click to reset. */
export function VFader({ label, min, max, value, reset, show, onChange }: {
  label: string; min: number; max: number; value: number; reset: number; show: (v: number) => string; onChange: (v: number) => void;
}) {
  const track = useRef<HTMLDivElement | null>(null);
  const span = max - min || 1;
  const k = (value - min) / span;
  const set = (clientY: number) => {
    const el = track.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const f = 1 - (clientY - r.top) / r.height;
    onChange(Math.round(min + Math.min(1, Math.max(0, f)) * span));
  };
  return (
    <div className="flex flex-col items-center gap-1">
      <div
        ref={track}
        role="slider"
        tabIndex={0}
        aria-label={label}
        aria-orientation="vertical"
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={value}
        aria-valuetext={show(value)}
        onPointerDown={(e) => { e.currentTarget.setPointerCapture(e.pointerId); set(e.clientY); }}
        onPointerMove={(e) => { if (e.buttons) set(e.clientY); }}
        onDoubleClick={() => onChange(reset)}
        onKeyDown={(e) => {
          const step = Math.max(1, Math.round(span / 20));
          if (e.key === "ArrowUp") { e.preventDefault(); onChange(Math.min(max, value + step)); }
          else if (e.key === "ArrowDown") { e.preventDefault(); onChange(Math.max(min, value - step)); }
        }}
        className="relative h-32 w-9 cursor-ns-resize touch-none rounded-[8px] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-text"
      >
        <span aria-hidden className="absolute inset-y-2 left-1/2 w-1 -translate-x-1/2 rounded-full bg-frost-300" />
        <span aria-hidden className="absolute bottom-2 left-1/2 w-1 -translate-x-1/2 rounded-full bg-primary-text/70" style={{ height: `calc(${k} * (100% - 16px))` }} />
        <span aria-hidden className="absolute left-0 h-4 w-9 rounded-[4px] bg-frost-300 shadow-[0_1px_2px_rgba(0,0,0,0.4)]" style={{ bottom: `calc(${k} * (100% - 16px))` }}>
          <span className="absolute inset-x-1.5 top-1/2 h-px -translate-y-1/2 bg-primary-text" />
        </span>
      </div>
      <span className="text-[11px] font-medium text-secondary-text">{label}</span>
      <span className="text-[10px] tabular-nums text-tertiary-text">{show(value)}</span>
    </div>
  );
}
