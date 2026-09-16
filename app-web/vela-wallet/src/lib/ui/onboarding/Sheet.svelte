<script lang="ts">
	/**
	 * Bottom-sheet overlay for web below the desktop breakpoint (spec 014,
	 * contract §3).
	 * Backdrop --color-fixed-backdrop, panel --color-bg-raised, top radius
	 * --radius-xl, enter/exit --motion-sheet-in/out, focus-trapped, aria-modal.
	 * Content hugs its own height per state (spec edge case).
	 *
	 * Drag-to-dismiss: the panel follows a downward pointer drag 1:1 (native
	 * sheet feel); release past 1/3 of the panel height OR a downward flick
	 * closes, anything else springs back. The backdrop dims proportionally.
	 * Sheets whose content scrolls keep native panning and dismiss via
	 * ×/Esc/backdrop only.
	 *
	 * Animation ownership: the ENTER keyframes carry no fill mode — their end
	 * state equals the natural state, and a lingering fill would permanently
	 * override the inline styles the drag writes. Once `animationend` fires
	 * the cascade is free and drags may engage. The EXIT keyframes keep
	 * `both` deliberately (they must hold the off-screen state until
	 * unmount); a close requested while the panel is drag-displaced routes
	 * through the inline slide-out instead, so the two drivers never fight.
	 */
	import type { Snippet } from 'svelte';

	interface Props {
		/** Accessible name for the dialog (resolved string). */
		label: string;
		/** Called after the exit animation completes (or immediately on unmount). */
		onClose: () => void;
		children: Snippet;
	}

	let { label, onClose, children }: Props = $props();

	let closing = $state(false);
	let panel = $state<HTMLElement | null>(null);
	/** True only while the pointer is actively driving the panel. */
	let dragging = $state(false);
	/** Backdrop dim level (1 = fully dimmed); inline, effective once entered. */
	let backdropDim = $state(1);
	/** Entry animation finished — the cascade is ours, drags may engage. */
	let entered = false;
	/** Set by the drag action: slides out from the current inline offset. */
	let dragSlideOut: (() => boolean) | null = null;
	let restoreFocus: HTMLElement | null = null;
	let suppressClick = false;
	let closed = false;

	function fireClose() {
		if (closed) return;
		closed = true;
		onClose();
	}

	/** Play the exit animation, then fire onClose (bindable from the host). */
	export function requestClose() {
		// A drag-displaced panel closes from where it is; the keyframe path
		// would snap it back to zero first.
		if (dragSlideOut?.()) return;
		closing = true;
	}

	$effect(() => {
		restoreFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
		panel?.focus();
		return () => restoreFocus?.focus();
	});

	function focusables(): HTMLElement[] {
		if (!panel) return [];
		return Array.from(
			panel.querySelectorAll<HTMLElement>(
				'a[href], button:not([disabled]), input:not([disabled]), select, textarea, [tabindex]:not([tabindex="-1"])'
			)
		);
	}

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			event.preventDefault();
			requestClose();
			return;
		}
		if (event.key !== 'Tab') return;
		const items = focusables();
		if (items.length === 0) {
			event.preventDefault();
			panel?.focus();
			return;
		}
		const first = items[0];
		const last = items[items.length - 1];
		const active = document.activeElement;
		if (event.shiftKey && (active === first || active === panel)) {
			event.preventDefault();
			last.focus();
		} else if (!event.shiftKey && active === last) {
			event.preventDefault();
			first.focus();
		}
	}

	function onPanelAnimationEnd(event: AnimationEvent) {
		if (event.target !== event.currentTarget) return;
		if (closing) {
			fireClose();
			return;
		}
		entered = true;
	}

	/** Swallow the synthetic click a finished drag would otherwise deliver. */
	function onPanelClickCapture(event: MouseEvent) {
		if (!suppressClick) return;
		suppressClick = false;
		event.stopPropagation();
		event.preventDefault();
	}

	function dragToDismiss(node: HTMLElement) {
		const SLOP = 8;
		const FLICK_VELOCITY = 0.5; // px per ms, downward
		let pointerId = -1;
		let startY = 0;
		let lastY = 0;
		let lastT = 0;
		let velocity = 0;
		let candidate = false;
		let engaged = false;

		const scrollable = () => node.scrollHeight > node.clientHeight + 1;
		const reducedMotion = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches;

		// Must be decided before a gesture starts: non-scrolling sheets own
		// vertical panning, scrolling ones keep it native.
		const applyTouchAction = () => {
			node.style.touchAction = scrollable() ? 'pan-y' : 'none';
		};
		applyTouchAction();
		const ro = new ResizeObserver(applyTouchAction);
		ro.observe(node);

		function slideOut() {
			if (reducedMotion()) {
				fireClose();
				return;
			}
			backdropDim = 0;
			node.style.transition = 'transform var(--motion-sheet-out) ease-in';
			void node.offsetHeight;
			node.style.transform = 'translateY(105%)';
			const done = (ev: TransitionEvent) => {
				if (ev.target !== node) return;
				node.removeEventListener('transitionend', done);
				fireClose();
			};
			node.addEventListener('transitionend', done);
			// Belt-and-braces: transitionend can be swallowed by unmounts.
			window.setTimeout(fireClose, 400);
		}

		function abortGesture() {
			if (engaged) {
				try {
					node.releasePointerCapture(pointerId);
				} catch {
					// pointer already gone
				}
			}
			candidate = false;
			engaged = false;
			dragging = false;
		}

		// requestClose() delegates here whenever the panel sits away from its
		// natural position (mid-drag or mid-settle), so the exit keyframes —
		// which always start from zero — are never asked to lie.
		dragSlideOut = () => {
			const displaced =
				engaged || (node.style.transform !== '' && node.style.transform !== 'translateY(0)');
			if (!displaced) return false;
			abortGesture();
			suppressClick = true;
			slideOut();
			return true;
		};

		function onDown(event: PointerEvent) {
			if (!entered || closing || engaged) return;
			if (event.pointerType === 'mouse' && event.button !== 0) return;
			if (scrollable() || node.scrollTop > 0) return;
			candidate = true;
			pointerId = event.pointerId;
			startY = lastY = event.clientY;
			lastT = event.timeStamp;
			velocity = 0;
		}

		function onMove(event: PointerEvent) {
			if (!candidate || event.pointerId !== pointerId) return;
			const dy = event.clientY - startY;
			const dt = event.timeStamp - lastT;
			if (dt > 0) velocity = (event.clientY - lastY) / dt;
			lastY = event.clientY;
			lastT = event.timeStamp;
			if (!engaged) {
				if (dy <= SLOP) return;
				engaged = true;
				dragging = true;
				node.setPointerCapture(pointerId);
				node.style.transition = 'none';
			}
			const y = Math.max(0, dy);
			node.style.transform = `translateY(${y}px)`;
			backdropDim = Math.max(0, 1 - y / Math.max(1, node.clientHeight));
			event.preventDefault();
		}

		function settle(event: PointerEvent, cancelled: boolean) {
			if (!candidate || event.pointerId !== pointerId) return;
			candidate = false;
			if (!engaged) return;
			engaged = false;
			dragging = false;
			suppressClick = true;
			const dy = Math.max(0, lastY - startY);
			const shouldClose = !cancelled && (dy > node.clientHeight / 3 || velocity > FLICK_VELOCITY);
			if (shouldClose) {
				slideOut();
			} else {
				backdropDim = 1;
				node.style.transition = 'transform var(--motion-sheet-in) ease-out';
				node.style.transform = 'translateY(0)';
				const done = (ev: TransitionEvent) => {
					if (ev.target !== node) return;
					node.removeEventListener('transitionend', done);
					node.style.transition = '';
					node.style.transform = '';
				};
				node.addEventListener('transitionend', done);
			}
		}

		const onUp = (event: PointerEvent) => settle(event, false);
		const onCancel = (event: PointerEvent) => settle(event, true);

		node.addEventListener('pointerdown', onDown);
		node.addEventListener('pointermove', onMove);
		node.addEventListener('pointerup', onUp);
		node.addEventListener('pointercancel', onCancel);
		return {
			destroy() {
				dragSlideOut = null;
				ro.disconnect();
				node.removeEventListener('pointerdown', onDown);
				node.removeEventListener('pointermove', onMove);
				node.removeEventListener('pointerup', onUp);
				node.removeEventListener('pointercancel', onCancel);
			}
		};
	}
</script>

<svelte:window onkeydown={onKeydown} />

<div class="overlay" class:closing>
	<button
		class="backdrop"
		type="button"
		tabindex="-1"
		aria-hidden="true"
		onclick={requestClose}
		style:opacity={backdropDim}
		style:transition={dragging ? 'none' : 'opacity var(--motion-sheet-in) ease-out'}
	></button>
	<div
		class="panel"
		role="dialog"
		aria-modal="true"
		aria-label={label}
		tabindex="-1"
		bind:this={panel}
		use:dragToDismiss
		onanimationend={onPanelAnimationEnd}
		onclickcapture={onPanelClickCapture}
	>
		{@render children()}
	</div>
</div>

<style>
	.overlay {
		position: fixed;
		inset: 0;
		z-index: 1;
		display: flex;
		align-items: flex-end;
		justify-content: center;
	}

	/* Enter keyframes carry NO fill: their end state IS the natural state,
	   and a forwards fill would permanently outrank the inline styles the
	   drag interaction writes (animations beat inline styles in the
	   cascade for as long as they apply). */
	.backdrop {
		position: absolute;
		inset: 0;
		border: none;
		padding: 0;
		background: var(--color-fixed-backdrop);
		animation: backdrop-in var(--motion-sheet-in) ease-out;
	}

	.closing .backdrop {
		animation: backdrop-out var(--motion-sheet-out) ease-in both;
	}

	/* The panel PADS ITS OWN CONTENT, the way `wallet/ui/BottomSheet` does
	   (`0 screenPaddingX screenPaddingX`).

	   It used not to, and three of the four callers never noticed: each pads
	   the `.card` of its DESKTOP branch and hands this one a bare snippet, so
	   below the breakpoint the sign-out sheet, the prompt sheet and the
	   identicon viewer all ran title, body and buttons flush into the screen
	   edge (measured: the panel padded nothing, `<h2>` at x=0). The fourth caller
	   — the welcome screen's method picker — had quietly added exactly this
	   rule to its own content, which is the tell: the padding belongs to the
	   sheet, not to whoever remembers. */
	.panel {
		position: relative;
		width: 100%;
		padding: var(--space-xl) var(--layout-screenPaddingX) var(--space-3xl);
		max-height: calc(100dvh - var(--space-5xl));
		overflow-y: auto;
		overscroll-behavior: contain;
		background: var(--color-bg-raised);
		border-start-start-radius: var(--radius-xl);
		border-start-end-radius: var(--radius-xl);
		animation: sheet-in var(--motion-sheet-in) ease-out;
	}

	.closing .panel {
		animation: sheet-out var(--motion-sheet-out) ease-in both;
	}

	@keyframes sheet-in {
		from {
			transform: translateY(100%);
		}
		to {
			transform: translateY(0);
		}
	}

	@keyframes sheet-out {
		from {
			transform: translateY(0);
		}
		to {
			transform: translateY(100%);
		}
	}

	@keyframes backdrop-in {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	@keyframes backdrop-out {
		from {
			opacity: 1;
		}
		to {
			opacity: 0;
		}
	}

	/* Desktop: the same surface as a centred dialog — a bottom sheet is a
	   phone's answer to a thumb, and on a desktop window it reads as one
	   (spec 038, founder ruling: 弹框 on every width). */
	@media (min-width: 1280px) {
		.overlay {
			align-items: center;
		}
		.panel {
			width: min(100%, calc(var(--layout-flowColumn) + var(--space-4xl) * 2));
			max-height: calc(100dvh - var(--space-5xl) * 2);
			border-radius: var(--radius-xl);
		}
	}
</style>
