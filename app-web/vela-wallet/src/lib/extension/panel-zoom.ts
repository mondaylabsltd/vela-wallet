/**
 * The side panel keeps its own scale (round 3, the owner's side panel).
 *
 * The owner's panel showed the home cut at its right edge — Scan, the
 * Settings tab, the Assets amounts — and the "Confirmed" landing drawn in the
 * right part of the panel with nothing on its left. No screen overflows at
 * any panel width (`extension-lifecycle.e2e.ts` measures 320 → 1400 in the
 * real panel), and the same pages in a tab were fine. What draws exactly
 * that picture is the panel ZOOMED IN: a trackpad pinch (Chrome hands it to
 * the page as a `wheel` with `ctrlKey`) enlarges the visual viewport inside
 * a strip a few hundred pixels wide. The layout does not reflow — the panel
 * shows only its left part, everything past it cut off — and a full-screen
 * layer like the landing is laid out on the whole page, so its middle sits
 * right of what the person can see. Nothing in the panel says it is zoomed.
 *
 * A pinch in the panel is therefore refused, as a native wallet refuses it
 * (and a mouse's Ctrl + wheel with it: the page cannot tell the two apart).
 * Chrome's own zoom (⌘/Ctrl + / −) is untouched: it REFLOWS the panel, so
 * larger text still fits the strip. Only the side panel: a tab is as wide as
 * its window, and zooming into it is the person's business.
 */

/** Refuse a pinch (trackpad `wheel` + `ctrlKey`; Safari's `gesture*`) on `target`. */
export function holdPanelScale(
	target: Pick<Window, 'addEventListener' | 'removeEventListener'>
): () => void {
	const wheel = (event: WheelEvent) => {
		if (event.ctrlKey) event.preventDefault();
	};
	const gesture = (event: Event) => event.preventDefault();
	target.addEventListener('wheel', wheel, { passive: false });
	target.addEventListener('gesturestart', gesture);
	target.addEventListener('gesturechange', gesture);
	return () => {
		target.removeEventListener('wheel', wheel);
		target.removeEventListener('gesturestart', gesture);
		target.removeEventListener('gesturechange', gesture);
	};
}
