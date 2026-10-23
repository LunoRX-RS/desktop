"use client";

import { cloneElement, useCallback, useEffect, useId, useRef, useState, type ReactElement, type ReactNode } from "react";
import { createPortal } from "react-dom";

type Place = { left: number; top: number; arrowLeft: number; below: boolean };

const GAP = 10;
const EDGE = 12;

/**
 * A tooltip for one trigger element. It opens on hover (after a short pause) and on keyboard focus, closes on leave, blur and Esc,
 * and is drawn in a portal so no scrolling or clipped parent can cut it off. It sits above the trigger, or below when there is no
 * room, and stays inside the window.
 */
export function Tip({ content, children }: { content: ReactNode; children: ReactElement<Record<string, unknown>> }) {
	const id = useId();
	const trigger = useRef<HTMLElement | null>(null);
	const bubble = useRef<HTMLDivElement>(null);
	const timer = useRef(0);
	const [open, setOpen] = useState(false);
	const [place, setPlace] = useState<Place | null>(null);

	const show = useCallback((delay: number) => {
		window.clearTimeout(timer.current);
		timer.current = window.setTimeout(() => setOpen(true), delay);
	}, []);
	const hide = useCallback(() => {
		window.clearTimeout(timer.current);
		setOpen(false);
		setPlace(null);
	}, []);

	// Measure after the bubble exists, so its real size decides where it goes.
	useEffect(() => {
		if (!open || !trigger.current || !bubble.current) return;
		const t = trigger.current.getBoundingClientRect();
		const b = bubble.current.getBoundingClientRect();
		const centre = t.left + t.width / 2;
		const left = Math.min(Math.max(centre - b.width / 2, EDGE), window.innerWidth - b.width - EDGE);
		const below = t.top - b.height - GAP < EDGE;
		const top = below ? t.bottom + GAP : t.top - b.height - GAP;
		setPlace({ left, top, below, arrowLeft: Math.min(Math.max(centre - left, 16), b.width - 16) });
	}, [open, content]);

	useEffect(() => {
		if (!open) return;
		const onKey = (e: KeyboardEvent) => e.key === "Escape" && hide();
		window.addEventListener("keydown", onKey);
		window.addEventListener("scroll", hide, true);
		return () => {
			window.removeEventListener("keydown", onKey);
			window.removeEventListener("scroll", hide, true);
		};
	}, [open, hide]);

	useEffect(() => () => window.clearTimeout(timer.current), []);

	const child = children;
	const trig = cloneElement(child, {
		ref: (node: HTMLElement | null) => {
			trigger.current = node;
		},
		"aria-describedby": open ? id : undefined,
		onMouseEnter: (e: unknown) => {
			show(120);
			(child.props.onMouseEnter as ((e: unknown) => void) | undefined)?.(e);
		},
		onMouseLeave: (e: unknown) => {
			hide();
			(child.props.onMouseLeave as ((e: unknown) => void) | undefined)?.(e);
		},
		onFocus: (e: unknown) => {
			show(0);
			(child.props.onFocus as ((e: unknown) => void) | undefined)?.(e);
		},
		onBlur: (e: unknown) => {
			hide();
			(child.props.onBlur as ((e: unknown) => void) | undefined)?.(e);
		},
	});

	return (
		<>
			{trig}
			{open &&
				createPortal(
					<div
						ref={bubble}
						id={id}
						role="tooltip"
						style={{ left: place?.left ?? 0, top: place?.top ?? 0, visibility: place ? "visible" : "hidden" }}
						className="lx-tip glass-solid [animation-delay:0ms] pointer-events-none fixed z-[80] w-max max-w-[300px] rounded-[12px] px-3.5 py-3 shadow-[0_16px_48px_rgba(0,0,0,0.55)]"
					>
						{content}
						<span
							aria-hidden="true"
							style={{ left: (place?.arrowLeft ?? 0) - 5 }}
							className={`absolute size-2.5 rotate-45 bg-elevated ${place?.below ? "-top-[5px]" : "-bottom-[5px]"}`}
						/>
					</div>,
					document.body,
				)}
		</>
	);
}
