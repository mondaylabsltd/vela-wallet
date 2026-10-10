<script module lang="ts">
	/**
	 * The open sheets, newest last. Escape and the Tab trap belong to the one
	 * on top: a viewer opened over the account switcher closes first, alone.
	 */
	const openSheets: symbol[] = [];
</script>

<script lang="ts">
	/**
	 * THE bottom sheet — every phone-width sheet in the app renders through
	 * this one (founder, 2026-09-27: "web 手机版本的 bottom sheet 弹框不一致，
	 * 有些支持下拉滑动关闭，有些不支持，能否统一用一个，就跟 android ios 一样").
	 * Four earlier shapes (this file, the onboarding sheet, the explore sheet
	 * and the signing sheet's own markup) behaved four ways: only one of them
	 * could be dragged closed, and that one had no grabber.
	 *
	 * One behaviour, the platform sheets' (numbers in `sheet-gesture.ts`):
	 *
	 * - **Grabber** at the top, always.
	 * - **Drag down** from the grabber or the header — or from the content, but
	 *   only a downward pull that BEGAN with the content at its top — and
	 *   release past ~28% of the height or with a downward flick: it closes.
	 *   Anything less springs back. The grabber dragged up rubber-bands; the
	 *   content pushed up never moves the sheet (an upward fling that hits the
	 *   content's top is swallowed, not handed on — the Android sheet's
	 *   endless top-anchor jitter). Nothing in the layout reads the drag
	 *   offset: it is a transform, and only a transform.
	 * - **Scrim tap, ✕ and Escape** close it the same way the drag does: one
	 *   close path, which plays the exit and THEN calls `onclose`, so the
	 *   route's state and what is on screen never disagree.
	 * - **`dismissible={false}`** while something must not be abandoned
	 *   mid-way (a signature in flight): the drag resists and comes back, and
	 *   the scrim, ✕ and Escape do nothing.
	 * - **`dismissible="explicit"`** for a sheet whose closing SAYS something
	 *   (spec 079 — the signing sheet and the consent card, where a close is
	 *   the dApp's 4001 and the site must ask again): only its ✕ (or the
	 *   host) closes it. No drag at all, no scrim tap, no Escape — the owner
	 *   lost a request to a stray touch ("除非用户明确关掉，不应该很容易误操作，
	 *   比如下滑就关掉了"). The grabber is not drawn: a handle on a sheet
	 *   that cannot be dragged is a control that cannot act.
	 * - **Focus** moves into the sheet, Tab stays in it, and goes back to
	 *   whatever opened it. **Reduced motion** drops every animation (the drag
	 *   still follows the finger — that is the person moving it, not motion).
	 * - **Safe area** at the bottom, and it **lifts above the on-screen
	 *   keyboard** (the visual viewport) so a field in a sheet stays visible.
	 *
	 * `variant` is the SKIN each family was drawn with — the behaviour above is
	 * identical in all of them:
	 *
	 * - `wallet` (default): title row + ✕, the wallet's page colour, `half` /
	 *   `tall` heights. Placed within the nearest positioned ancestor.
	 * - `menu`: the explore menus — no title row, 78% tall.
	 * - `signing`: the signing sheet, raised, above the page (fixed); past the
	 *   desktop breakpoint it is a centred card with nothing to drag (founder
	 *   ruling 2026-09-05: no bottom sheets on desktop).
	 * - `prompt`: the onboarding prompts — sign-out, the identicon viewer, the
	 *   welcome picker. Raised, hugs its content, fixed; a centred dialog past
	 *   the breakpoint.
	 */
	import { onDestroy, type Snippet } from 'svelte';
	import { UTILITY_ICONS } from '../icons';
	import Icon from './Icon.svelte';
	import {
		contentTakesDrag,
		scrimOpacity,
		sheetOffset,
		shouldDismiss,
		SLOP,
		VelocityTracker
	} from './sheet-gesture';

	interface Props {
		/** The dialog's accessible name, and its visible title unless `hideTitle`. */
		title: string;
		/** Optional trailing icon-button slot in the title row (mock H8: search). */
		trailingIcon?: 'search';
		/** No visible title row, only the a11y name (action sheets, and every non-wallet skin). */
		hideTitle?: boolean;
		/**
		 * Spec 021: a close button in the title row — every sheet in the
		 * wallet-2 mocks draws an explicit ×, and a sheet reached mid-transfer
		 * needs a way out that does not depend on knowing a gesture.
		 */
		closeLabel?: string;
		/** `wallet` skin only: spec 015's 60%, or spec 021's 88% for sheets whose content IS the screen. */
		height?: 'half' | 'tall';
		/** The skin; the behaviour is the same in every one. */
		variant?: 'wallet' | 'menu' | 'signing' | 'prompt';
		/**
		 * `false` while it must not close (a signature in flight): drags resist,
		 * nothing closes it. `'explicit'`: only the ✕ (or the host) closes it —
		 * no drag, no scrim, no Escape (spec 079).
		 */
		dismissible?: boolean | 'explicit';
		/** After the exit plays — however the sheet was dismissed. */
		onclose?: () => void;
		children: Snippet;
	}

	let {
		title,
		trailingIcon,
		hideTitle = false,
		closeLabel,
		height = 'half',
		variant = 'wallet',
		dismissible = true,
		onclose,
		children
	}: Props = $props();

	const self = Symbol('sheet');
	let panel = $state<HTMLElement>();
	let scroller = $state<HTMLElement>();

	/** px below rest (negative = above) while dragged or settling. */
	let offset = $state(0);
	/** A finger (or the mouse) is moving the sheet. */
	let dragging = $state(false);
	/** After a drag: easing back to rest, or out. */
	let settling = $state<'none' | 'back' | 'out'>('none');
	/** The entry animation is done with — a drag has taken the transform over. */
	let entered = $state(false);
	/** The on-screen keyboard's height over the page, from the visual viewport. */
	let keyboard = $state(0);

	let closing = false;
	let alive = true;
	let suppressClick = false;
	let settleTimer: ReturnType<typeof setTimeout> | undefined;
	const tracker = new VelocityTracker();

	const reducedMotion = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches;
	/** Past the breakpoint the signing and prompt skins are cards: nothing to drag. */
	const isCard = () =>
		(variant === 'signing' || variant === 'prompt') &&
		window.matchMedia('(min-width: 1280px)').matches;
	const panelHeight = () => panel?.getBoundingClientRect().height ?? 0;
	const onTop = () => openSheets[openSheets.length - 1] === self;

	// --- the one close path ------------------------------------------------

	function finish(): void {
		clearTimeout(settleTimer);
		if (alive) onclose?.();
	}

	/**
	 * Which door a close came through. `gesture` — the drag, the scrim,
	 * Escape: the ones a person can use without meaning to. `explicit` — the
	 * ✕, a control that says what it does. `host` — the host's own "close now"
	 * (a confirm button), which a non-dismissible sheet still obeys.
	 */
	type Door = 'gesture' | 'explicit' | 'host';

	/** Whether `door` may close the sheet in its current mode. */
	function opens(door: Door): boolean {
		if (door === 'host') return true;
		// Nobody to tell (a gallery picture of a sheet has no host to take it down).
		if (onclose === undefined || dismissible === false) return false;
		return door === 'explicit' || dismissible === true;
	}

	/** Only the ✕ and the host close it (spec 079): no drag is ever taken. */
	const explicitOnly = $derived(dismissible === 'explicit');
	/** The drag's own gate — anything but a free-closing sheet resists. */
	const dragCloses = $derived(dismissible === true);

	/**
	 * Close, the way every door does: the exit plays from wherever the sheet
	 * is, then `onclose`.
	 */
	function dismiss(door: Door = 'gesture'): void {
		if (closing) return;
		// A door this mode keeps shut: it resists and comes back.
		if (!opens(door)) {
			if (offset !== 0) springBack();
			return;
		}
		closing = true;
		dragging = false;
		// A card past the breakpoint that never slid in does not slide out.
		if (reducedMotion() || panel === undefined || (variant === 'signing' && isCard())) {
			finish();
			return;
		}
		entered = true;
		settling = 'out';
		offset = panelHeight() + keyboard;
		settleTimer = setTimeout(finish, 420);
	}

	/** The host asks for the exit (its own Cancel / Confirm buttons). */
	export function requestClose(): void {
		dismiss('host');
	}

	/**
	 * An explicit close drawn by the CONTENT rather than the title row — the
	 * signing sheet's ✕ sits in its own header (spec 079). It goes through the
	 * same door as the title row's ✕: obeyed unless the sheet is locked
	 * (`dismissible={false}`), and after the exit plays.
	 */
	export function close(): void {
		dismiss('explicit');
	}

	// --- keeping one thing where it is --------------------------------------

	/**
	 * px the centred card is raised from its own middle (`keepAt`). A card
	 * grows from its middle — both edges move — so what stands at its foot
	 * goes down by half of whatever was added above it.
	 */
	let lift = $state(0);

	/**
	 * Where the first `selector` in this sheet RESTS on the screen (its top),
	 * whatever the entry animation or a drag is doing to the sheet at this
	 * instant. `null` when it is not drawn.
	 */
	export function placeOf(selector: string): number | null {
		const target = panel?.querySelector(selector);
		if (!panel || !target) return null;
		// The sheet's laid-out top, then the target's place inside it: the
		// difference of two boxes under one transform is no transform at all.
		const inside = target.getBoundingClientRect().top - panel.getBoundingClientRect().top;
		return panel.offsetTop - lift + inside;
	}

	/**
	 * Put the first `selector` back where it rested (`top`, from `placeOf`)
	 * after the sheet's content changed above it (PR 3 final note F2).
	 *
	 * The phone sheet is bottom-anchored: content added above its foot grows
	 * it upward and the foot stays — until the sheet is as tall as it may be.
	 * Then it scrolls, and the foot goes down under what was added: scrolled
	 * by that much, it is where it was. Past the breakpoint the sheet is a
	 * centred card that grows from its middle: raised by what scrolling could
	 * not take, it grows upward only.
	 *
	 * `show` is never scrolled out of sight for it — what was added is there
	 * to be read — nor under `under`, a header the content keeps at its top
	 * (the foot then moves by the rest).
	 */
	export function keepAt(
		selector: string,
		top: number,
		keep: { show?: string; under?: string } = {}
	): void {
		const now = placeOf(selector);
		if (!panel || now === null) return;
		let drift = now - top;
		if (Math.abs(drift) < 0.5) return;
		if (scroller) {
			const from = scroller.scrollTop;
			scroller.scrollTop = from + drift;
			const added = keep.show === undefined ? null : panel.querySelector(keep.show);
			if (added) {
				const cover = keep.under === undefined ? null : panel.querySelector(keep.under);
				const edge = Math.max(
					scroller.getBoundingClientRect().top,
					cover?.getBoundingClientRect().bottom ?? 0
				);
				const hidden = edge - added.getBoundingClientRect().top;
				if (hidden > 0) scroller.scrollTop = Math.max(from, scroller.scrollTop - hidden);
			}
			drift -= scroller.scrollTop - from;
		}
		if (Math.abs(drift) < 0.5 || !isCard()) return;
		// Never past the room the card has above or below it: its own margin
		// from the window's edge (`--space-3xl`, as its max-height keeps).
		const margin = parseFloat(getComputedStyle(panel).getPropertyValue('--space-3xl')) || 0;
		const room = Math.max(0, panel.offsetTop - margin);
		lift = Math.min(room, Math.max(-room, lift + drift));
	}

	/** Back to its middle: the window changed, or what the sheet holds did. */
	export function recentre(): void {
		lift = 0;
	}

	function springBack(): void {
		if (offset === 0) {
			settling = 'none';
			return;
		}
		if (reducedMotion()) {
			offset = 0;
			settling = 'none';
			return;
		}
		settling = 'back';
		offset = 0;
		clearTimeout(settleTimer);
		settleTimer = setTimeout(() => (settling = 'none'), 420);
	}

	function onPanelTransitionEnd(event: TransitionEvent): void {
		if (event.target !== panel || event.propertyName !== 'transform') return;
		if (settling === 'out') finish();
		else if (settling === 'back') {
			clearTimeout(settleTimer);
			settling = 'none';
		}
	}

	// --- the drag ------------------------------------------------------------

	function engage(): void {
		entered = true;
		dragging = true;
		settling = 'none';
		clearTimeout(settleTimer);
	}

	function follow(dy: number): void {
		offset = sheetOffset(dy, panelHeight(), dragCloses);
	}

	function release(cancelled: boolean): void {
		dragging = false;
		suppressClickBriefly();
		const close =
			!cancelled &&
			shouldDismiss({
				offset,
				height: panelHeight(),
				velocity: tracker.velocity(),
				dismissible: dragCloses
			});
		if (close) dismiss();
		else springBack();
	}

	function suppressClickBriefly(): void {
		suppressClick = true;
		setTimeout(() => (suppressClick = false), 400);
	}

	/** Swallow the click a finished drag would otherwise deliver to what it started on. */
	function onPanelClickCapture(event: MouseEvent): void {
		if (!suppressClick) return;
		suppressClick = false;
		event.stopPropagation();
		event.preventDefault();
	}

	// The grabber and the header: always the sheet's to drag (pointer events —
	// touch, pen and mouse alike). Listened for on the panel, so the drag
	// carries on wherever the finger goes; only a press that STARTS on the
	// grabber or the title row begins one.
	let pointer: { id: number; startY: number; engaged: boolean } | null = null;

	function onGripDown(event: PointerEvent): void {
		if (closing || isCard() || explicitOnly) return;
		if (event.pointerType === 'mouse' && event.button !== 0) return;
		const target = event.target instanceof Element ? event.target : null;
		if (!target?.closest('[data-sheet-grip]')) return;
		pointer = { id: event.pointerId, startY: event.clientY, engaged: false };
		tracker.reset(event.timeStamp, event.clientY);
	}

	function onGripMove(event: PointerEvent): void {
		if (pointer === null || event.pointerId !== pointer.id) return;
		const dy = event.clientY - pointer.startY;
		tracker.add(event.timeStamp, event.clientY);
		if (!pointer.engaged) {
			if (Math.abs(dy) < SLOP) return;
			pointer.engaged = true;
			engage();
			try {
				panel?.setPointerCapture(event.pointerId);
			} catch {
				// the pointer is already gone
			}
		}
		event.preventDefault();
		follow(dy);
	}

	function onGripEnd(event: PointerEvent): void {
		if (pointer === null || event.pointerId !== pointer.id) return;
		const engaged = pointer.engaged;
		pointer = null;
		if (engaged) release(event.type === 'pointercancel');
	}

	// The content: the sheet's only when the content cannot use the movement
	// itself (touch events, so the pull can be claimed before the browser
	// scrolls or over-scrolls; a mouse scrolls content with its wheel).
	let touch: {
		startX: number;
		startY: number;
		decided: 'sheet' | 'content' | null;
		atTop: boolean;
		owned: boolean;
	} | null = null;

	/** Every scroller between the finger and the sheet is at its top. */
	function atTopUnder(target: Element | null): boolean {
		for (let el = target; el && el !== panel; el = el.parentElement) {
			if (el.scrollTop > 0) return false;
			if (el === scroller) break;
		}
		return true;
	}

	/**
	 * A control that owns its own touch gesture keeps it: text fields (a drag
	 * there selects text), and anything that says so with `touch-action` —
	 * a horizontal slider.
	 */
	function ownsGesture(target: Element | null): boolean {
		if (target?.closest('input, textarea, select, [contenteditable], [data-sheet-nodrag]')) {
			return true;
		}
		for (let el = target; el && el !== scroller; el = el.parentElement) {
			const action = getComputedStyle(el).touchAction;
			if (action === 'none' || action.includes('pan-x')) return true;
		}
		return false;
	}

	function onTouchStart(event: TouchEvent): void {
		if (
			closing ||
			isCard() ||
			explicitOnly ||
			event.touches.length !== 1 ||
			scroller === undefined
		) {
			touch = null;
			return;
		}
		const point = event.touches[0];
		const target = event.target instanceof Element ? event.target : null;
		touch = {
			startX: point.clientX,
			startY: point.clientY,
			decided: null,
			atTop: atTopUnder(target),
			owned: ownsGesture(target)
		};
		tracker.reset(event.timeStamp, point.clientY);
	}

	function onTouchMove(event: TouchEvent): void {
		const t = touch;
		if (t === null || event.touches.length !== 1) return;
		const point = event.touches[0];
		const dx = point.clientX - t.startX;
		const dy = point.clientY - t.startY;
		tracker.add(event.timeStamp, point.clientY);
		if (t.decided === 'content') return;
		if (t.decided === null) {
			if (t.owned) {
				t.decided = 'content';
				return;
			}
			const verdict = contentTakesDrag({ dx, dy, atTop: t.atTop });
			if (verdict === 'undecided') {
				// Pulling down at the top: hold the page still (no over-scroll,
				// no pull-to-refresh) while the gesture makes up its mind.
				if (dy > 0 && t.atTop && event.cancelable) event.preventDefault();
				return;
			}
			t.decided = verdict;
			if (verdict === 'content') return;
			engage();
		}
		if (event.cancelable) event.preventDefault();
		follow(dy);
	}

	function onTouchEnd(event: TouchEvent): void {
		const t = touch;
		touch = null;
		if (t?.decided === 'sheet') release(event.type === 'touchcancel');
	}

	$effect(() => {
		const node = scroller;
		if (node === undefined) return;
		// Non-passive: claiming the pull means cancelling the scroll it would start.
		node.addEventListener('touchstart', onTouchStart, { passive: true });
		node.addEventListener('touchmove', onTouchMove, { passive: false });
		node.addEventListener('touchend', onTouchEnd);
		node.addEventListener('touchcancel', onTouchEnd);
		return () => {
			node.removeEventListener('touchstart', onTouchStart);
			node.removeEventListener('touchmove', onTouchMove);
			node.removeEventListener('touchend', onTouchEnd);
			node.removeEventListener('touchcancel', onTouchEnd);
		};
	});

	// --- focus, Escape, the keyboard ------------------------------------------

	function focusables(): HTMLElement[] {
		if (panel === undefined) return [];
		return Array.from(
			panel.querySelectorAll<HTMLElement>(
				'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
			)
		).filter((el) => el.offsetParent !== null || el === document.activeElement);
	}

	function onKeydown(event: KeyboardEvent): void {
		if (!onTop()) return;
		if (event.key === 'Escape') {
			// Escape during a Chinese/Japanese composition cancels the
			// composition — it is not a request to leave.
			if (event.isComposing || event.keyCode === 229) return;
			event.preventDefault();
			dismiss();
			return;
		}
		if (event.key !== 'Tab' || panel === undefined) return;
		const items = focusables();
		if (items.length === 0) {
			event.preventDefault();
			panel.focus({ preventScroll: true });
			return;
		}
		const first = items[0];
		const last = items[items.length - 1];
		const active = document.activeElement;
		const inside = active instanceof Node && panel.contains(active);
		if (event.shiftKey && (active === first || active === panel || !inside)) {
			event.preventDefault();
			last.focus();
		} else if (!event.shiftKey && (active === last || !inside)) {
			event.preventDefault();
			first.focus();
		}
	}

	$effect(() => {
		const node = panel;
		if (node === undefined) return;
		openSheets.push(self);
		const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
		// Content that focused something of its own (a field) keeps it.
		if (!node.contains(document.activeElement)) node.focus({ preventScroll: true });
		return () => {
			const at = openSheets.indexOf(self);
			if (at !== -1) openSheets.splice(at, 1);
			const active = document.activeElement;
			const lost = active === null || active === document.body || node.contains(active);
			if (lost && opener?.isConnected) opener.focus({ preventScroll: true });
		};
	});

	$effect(() => {
		const viewport = window.visualViewport;
		if (!viewport) return;
		const measure = () => {
			keyboard = Math.max(0, window.innerHeight - viewport.height - viewport.offsetTop);
		};
		measure();
		viewport.addEventListener('resize', measure);
		viewport.addEventListener('scroll', measure);
		return () => {
			viewport.removeEventListener('resize', measure);
			viewport.removeEventListener('scroll', measure);
		};
	});

	onDestroy(() => {
		alive = false;
		clearTimeout(settleTimer);
	});

	const transition = $derived(
		dragging
			? 'none'
			: settling === 'out'
				? 'transform var(--motion-sheet-out) ease-in'
				: settling === 'back'
					? 'transform var(--motion-sheet-drag) ease-out'
					: undefined
	);
	const scrimLevel = $derived(settling === 'out' ? 0 : scrimOpacity(offset, panelHeight()));
</script>

<svelte:window onkeydown={onKeydown} onresize={recentre} />

<div
	class="root {variant} {variant === 'wallet' ? height : ''}"
	class:closing={settling === 'out'}
	style:--sheet-keyboard={keyboard > 0 ? `${keyboard}px` : undefined}
>
	<div
		class="scrim"
		role="presentation"
		onclick={() => dismiss()}
		style:opacity={scrimLevel === 1 ? undefined : scrimLevel}
		style:transition={dragging
			? 'none'
			: settling !== 'none'
				? 'opacity var(--motion-sheet-out) ease-in'
				: undefined}
	></div>
	<div
		class="sheet"
		class:entered
		role="dialog"
		aria-modal="true"
		aria-label={title}
		tabindex="-1"
		data-focus-inner
		bind:this={panel}
		style:transform={offset !== 0 || settling !== 'none' ? `translateY(${offset}px)` : undefined}
		style:translate={lift !== 0 ? `0 ${-lift}px` : undefined}
		style:transition
		ontransitionend={onPanelTransitionEnd}
		onclickcapture={onPanelClickCapture}
		onpointerdown={onGripDown}
		onpointermove={onGripMove}
		onpointerup={onGripEnd}
		onpointercancel={onGripEnd}
	>
		<!-- No grabber on a sheet that cannot be dragged (`explicit`): the space
		     stays, so the layout above the content does not move. -->
		<span class="handle" class:inert={explicitOnly} aria-hidden="true" data-sheet-grip></span>
		{#if !hideTitle}
			<header data-sheet-grip>
				<h2>{title}</h2>
				{#if trailingIcon === 'search'}
					<span class="trailing"><Icon icon={UTILITY_ICONS.search} size="lg" /></span>
				{/if}
				{#if closeLabel !== undefined}
					<button
						type="button"
						class="close"
						aria-label={closeLabel}
						disabled={dismissible === false}
						onclick={() => dismiss('explicit')}
					>
						<Icon icon={UTILITY_ICONS.x} size="lg" />
					</button>
				{/if}
			</header>
		{/if}
		<div class="content" bind:this={scroller}>
			{@render children()}
		</div>
	</div>
</div>

<style>
	/* --- layers ------------------------------------------------------------ */

	/* `wallet` and `menu` sit within the nearest positioned ancestor (the
	   phone frame, the page); the root adds no box of its own. */
	.root {
		display: contents;
	}

	/* `signing` and `prompt` are above the page wherever they are mounted:
	   FIXED, stacked where the signing sheet always was (20/21), so the
	   dApp guard (19) stays under it and the receipt above. */
	.root.signing,
	.root.prompt {
		position: fixed;
		inset: 0;
		z-index: 20;
		display: block;
	}

	.scrim {
		position: absolute;
		inset: 0;
		background: var(--color-fixed-backdrop);
		animation: scrim-in var(--motion-sheet-in) ease-out;
	}

	.closing .scrim {
		pointer-events: none;
	}

	/* --- the sheet --------------------------------------------------------- */

	/* The ENTRY keyframes carry no fill: their end state IS the resting state,
	   and a lingering fill would outrank the transform the drag writes. Once a
	   drag starts, `.entered` drops the animation altogether. */
	.sheet {
		position: absolute;
		inset-inline: 0;
		bottom: var(--sheet-keyboard, 0%);
		display: flex;
		flex-direction: column;
		outline: none;
		animation: sheet-in var(--motion-sheet-in) ease-out;
	}

	.sheet.entered {
		animation: none;
	}

	/* The grabber and the title row are the sheet's to drag: no browser
	   panning there, the finger is ours from the first pixel. */
	.handle,
	header {
		touch-action: none;
	}

	.handle {
		position: relative;
		align-self: center;
		flex: none;
		width: var(--space-5xl);
		height: var(--space-sm);
		border-radius: var(--radius-full);
		background: var(--color-border-strong);
		margin-block: var(--space-lg);
		cursor: grab;
	}

	/* The pill is 4 tall; the place to grab it is not. The pseudo-element is
	   part of the pill for hit-testing and draws nothing, so the grab area
	   fills the margin band above and below and reaches well to either side
	   — without adding a box that would move the layout. */
	.handle.inert {
		visibility: hidden;
		cursor: default;
	}

	.handle::after {
		content: '';
		position: absolute;
		inset: calc(-1 * var(--space-lg)) calc(-1 * var(--space-5xl));
	}

	.content {
		overflow-x: clip;
		overflow-y: auto;
		/* A sheet's scroll never carries on into the page under it. */
		overscroll-behavior: contain;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-block: var(--space-md);
	}

	h2 {
		margin: 0;
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.trailing {
		color: var(--color-fg-muted);
		display: flex;
	}

	.close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		margin-inline-end: calc(var(--space-md) * -1);
		border: none;
		border-radius: var(--radius-full);
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.close:hover:not(:disabled) {
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
	}

	.close:disabled {
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	/* --- skins --------------------------------------------------------------- */

	/* wallet: the page colour, a title row, the gutter on the sheet. */
	.wallet .sheet {
		background: var(--color-bg-base);
		border-start-start-radius: var(--radius-2xl);
		border-start-end-radius: var(--radius-2xl);
		padding-inline: var(--layout-screenPaddingX);
		padding-bottom: calc(var(--space-3xl) + env(safe-area-inset-bottom, 0%));
	}

	.half .sheet {
		max-height: calc(60% - var(--sheet-keyboard, 0%));
	}

	.tall .sheet {
		max-height: calc(88% - var(--sheet-keyboard, 0%));
	}

	/* The scroll box spans the whole sheet, its content still on the gutter
	   column: whatever reaches a little past the column — a tap area around
	   a corner badge, a focus ring — lands in the gutter, still inside this
	   box, instead of being cut off or scrolling the sheet sideways. */
	.wallet .content {
		margin-inline: calc(-1 * var(--layout-screenPaddingX));
		padding-inline: var(--layout-screenPaddingX);
	}

	/* menu, signing: the gutter and the bottom space scroll with the content. */
	.menu .sheet,
	.signing .sheet {
		border-start-start-radius: var(--radius-2xl);
		border-start-end-radius: var(--radius-2xl);
	}

	.menu .sheet {
		max-height: calc(78% - var(--sheet-keyboard, 0%));
		background: var(--color-bg-base);
	}

	.signing .sheet {
		max-height: calc(88% - var(--sheet-keyboard, 0%));
		background: var(--color-bg-raised);
	}

	.menu .content,
	.signing .content,
	.prompt .content {
		padding-inline: var(--layout-screenPaddingX);
		padding-bottom: calc(var(--space-3xl) + env(safe-area-inset-bottom, 0%));
	}

	/* prompt: raised, hugging its content. */
	.prompt .sheet {
		max-height: calc(100dvh - var(--space-5xl) - var(--sheet-keyboard, 0%));
		background: var(--color-bg-raised);
		border-start-start-radius: var(--radius-xl);
		border-start-end-radius: var(--radius-xl);
	}

	/* --- motion -------------------------------------------------------------- */

	@keyframes sheet-in {
		from {
			transform: translateY(100%);
		}
		to {
			transform: translateY(0);
		}
	}

	@keyframes scrim-in {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	@keyframes card-in {
		from {
			transform: translateY(var(--space-5xl));
			opacity: 0;
		}
		to {
			transform: translateY(0);
			opacity: 1;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.sheet,
		.scrim {
			animation: none;
		}
	}

	/* --- desktop ------------------------------------------------------------- */

	/* Past the breakpoint a bottom sheet is the wrong object (founder ruling
	   2026-09-05). The two skins that can reach a desktop window become the
	   centred cards they always were there, with nothing to drag. */
	@media (min-width: 1280px) {
		.signing .sheet,
		.prompt .sheet {
			inset: 0;
			margin: auto;
			height: fit-content;
			border-radius: var(--radius-xl);
		}

		.signing .handle,
		.prompt .handle {
			display: none;
		}

		.signing .sheet {
			width: calc(100% - 2 * var(--space-3xl));
			max-width: var(--layout-promptCard);
			max-height: calc(100% - 2 * var(--space-3xl));
			padding-top: var(--space-2xl);
			border: var(--border-hairline) solid var(--color-border-base);
			box-shadow: var(--shadow-lg);
			animation: card-in var(--motion-sheet-in) ease-out;
		}

		.prompt .sheet {
			width: min(100%, calc(var(--layout-flowColumn) + var(--space-4xl) * 2));
			max-height: calc(100dvh - var(--space-5xl) * 2);
			padding-top: var(--space-xl);
		}
	}

	@media (min-width: 1280px) and (prefers-reduced-motion: reduce) {
		.signing .sheet {
			animation: none;
		}
	}
</style>
