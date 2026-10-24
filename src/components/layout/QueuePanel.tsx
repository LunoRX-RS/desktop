"use client";

import { motion, useReducedMotion } from "motion/react";
import NativeQueue from "../../features/queue/NativeQueue";
import QueueScreen from "../../features/queue/QueueScreen";
import { usePlayback } from "../../lib/playback";
import { DUR, SPRING, tween } from "../../lib/motion";
import { cx } from "../ui";

/**
 * The queue in a floating panel with room around it on the top, right and bottom. In the app it floats over the page. Now Playing
 * is a modal dialog that covers everything outside itself, so there the panel is drawn inside the dialog (`inDialog`) and stops
 * above the controls pill.
 */
export function QueuePanel({ inDialog = false }: { inDialog?: boolean }) {
	const reduce = useReducedMotion();
	const native = usePlayback((p) => p.requested !== null || p.track !== null || p.resume !== null);
	return (
		<motion.aside
			key="queue"
			aria-label={native ? "Queue" : "Queue (demo)"}
			// Slides in from the right edge on a spring and settles; only transform and opacity change, so it stays on the GPU.
			initial={{ opacity: 0, x: reduce ? 0 : 40, scale: reduce ? 1 : 0.98 }}
			animate={{ opacity: 1, x: 0, scale: 1, transition: { ...SPRING, opacity: tween(DUR.base) } }}
			exit={{ opacity: 0, x: reduce ? 0 : 32, transition: tween(DUR.exit) }}
			style={{ transformOrigin: "100% 50%" }}
			className={cx(
				"glass-solid transform-gpu will-change-transform [backface-visibility:hidden] w-[min(440px,calc(100vw-48px))] overflow-y-auto overscroll-contain rounded-[12px] p-4 shadow-[0_16px_48px_rgba(0,0,0,0.5)]",
				inDialog
					? "absolute top-6 right-6 bottom-[148px] z-20"
					: "fixed top-[calc(var(--height-topbar)+12px)] right-6 bottom-[calc(var(--height-player)+24px)] z-[45]",
			)}
		>
			{native ? <NativeQueue /> : <QueueScreen />}
		</motion.aside>
	);
}
