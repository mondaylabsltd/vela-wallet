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
		/**
		 * What stays at the foot of the sheet, OUTSIDE its scroll (PR 3 device
		 * round): the signing sheet's confirm and the line under it. The body
		 * above scrolls when the sheet is as tall as it may be; this never
		 * scrolls, is never covered, and is not moved by what the body holds.
		 * It carries the sheet's gutter and its bottom space (the safe area), so
		 * a sheet that fits is drawn exactly as if this were its last content.
		 */
		footer?: Snippet;
		/**
		 * The words of the cue `reveal` shows instead of scrolling, once the
		 * person has scrolled the body themselves: "New check result below ↓".
		 * Without them `reveal` always scrolls — what landed is never left out
		 * of sight with nothing to say so.
		 */
		revealCue?: { below: string; above: string };
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
		footer,
		revealCue,
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
	 * The centred card's foot, held (`keepAt`): how far its bottom edge is
	 * from the window's, and how tall it may be with its foot there. A card
	 * grows from its middle — both edges move — so what stands at its foot
	 * goes down by half of whatever lands above it; held, it grows upward
	 * only, as the phone sheet does.
	 */
	let held = $state<{ bottom: number; most: number } | null>(null);

	/**
	 * Where the first `selector` in this sheet RESTS on the screen (its top),
	 * whatever the entry animation or a drag is doing to the sheet at this
	 * instant. `null` when it is not drawn.
	 */
	export function placeOf(selector: string): number | null {
		const target = panel?.querySelector(selector);
		const frame = panel?.offsetParent;
		if (!panel || !target || !frame) return null;
		// Where it is drawn, less what is moving the sheet at this instant (the
		// entry animation's transform, or a drag's). Not `offsetTop`: that is
		// a whole number, and a card centred in a window sits on a half pixel.
		const moving = getComputedStyle(panel).transform;
		const shift = moving === 'none' ? 0 : new DOMMatrixReadOnly(moving).m42;
		return target.getBoundingClientRect().top - shift - frame.getBoundingClientRect().top;
	}

	/**
	 * Keep the first `selector` — something in the sheet's `footer` — where
	 * it rested (`top`, from `placeOf`) after the body's content changed
	 * (PR 3 final note F2; the device round's pinned foot).
	 *
	 * The phone sheet is bottom-anchored and its foot is outside the scroll:
	 * content added to the body grows the sheet upward, then scrolls the
	 * body, and the foot stays — there is nothing to do. Past the breakpoint
	 * the sheet is a centred card that grows from its middle: from here on its
	 * foot is held where it rested, so the card grows upward only, to its own
	 * margin from the window's edge, and then its body scrolls.
	 */
	export function keepAt(selector: string, top: number): void {
		const target = panel?.querySelector(selector);
		const now = placeOf(selector);
		if (!panel || !target || now === null) return;
		// Anything measurable: a card a fraction short of its tallest still
		// goes down by half of that fraction.
		if (Math.abs(now - top) < 0.01 || !isCard()) return;
		const frame = panel.offsetParent;
		if (!frame) return;
		// Where the card's bottom edge was when the target rested at `top`.
		const foot = top + panel.getBoundingClientRect().bottom - target.getBoundingClientRect().top;
		// No higher than its top stands when it is as tall as it may be: the
		// stylesheet's `max-height` leaves `--space-3xl` at either edge, and
		// limits the height INSIDE the card's own padding and border.
		const style = getComputedStyle(panel);
		const margin = parseFloat(style.getPropertyValue('--space-3xl')) || 0;
		const chrome =
			style.boxSizing === 'border-box'
				? 0
				: parseFloat(style.paddingTop) +
					parseFloat(style.paddingBottom) +
					parseFloat(style.borderTopWidth) +
					parseFloat(style.borderBottomWidth);
		const highest = margin - chrome / 2;
		held = {
			bottom: frame.clientHeight - foot,
			most: Math.max(0, foot - highest - chrome)
		};
	}

	// --- bringing what landed into sight ------------------------------------

	/** One thing `reveal` was asked to bring into sight. */
	interface Ask {
		selector: string;
		nth: number;
		under?: string;
	}

	/**
	 * How long the body must have been left alone — no finger, no press, no
	 * wheel, no scrolling key, no scroll still coasting — before a reveal
	 * asked for meanwhile is carried out.
	 */
	const SETTLE_MS = 300;

	/**
	 * The person's hand on the body now: a finger on it, a press on it (the
	 * mouse, a pen, its scrollbar), a scrolling key held down.
	 */
	const hand = { touch: false, press: false, key: false };
	/** Running from the last of the person's scrolling input: not yet settled. */
	let quiet: ReturnType<typeof setTimeout> | undefined;
	/** The person has scrolled the body themselves since the sheet opened. */
	let scrolledByHand = false;
	/** Asked for while the person's hand was on the body: carried out once it settles. */
	let asked: Ask[] = [];
	let askedSoon = false;
	/**
	 * What landed out of sight after the person had scrolled away from it:
	 * the cue stands until each is in sight. The last one is where a tap on
	 * the cue ends, and which way the cue points.
	 */
	let owed = $state<Ask[]>([]);
	let cueSide = $state<'below' | 'above'>('below');
	let cueButton = $state<HTMLButtonElement>();

	const handOn = () => hand.touch || hand.press || hand.key || quiet !== undefined;

	/** The person's input moved (or let go): the settle starts again. */
	function stir(): void {
		clearTimeout(quiet);
		quiet = setTimeout(() => {
			quiet = undefined;
			if (!hand.touch && !hand.press && !hand.key) settle();
		}, SETTLE_MS);
	}

	function settle(): void {
		const asks = asked;
		asked = [];
		if (asks.length > 0) bring(asks);
	}

	/**
	 * Where the `ask`ed element stands against the body's window, as if the
	 * body were scrolled to `from`: `null` when it is not in the body. The
	 * window's top is under the `under` header, which the content keeps at
	 * the top of the scroll (it does not move with the scroll).
	 */
	function measure(ask: Ask, from: number) {
		const target = panel?.querySelectorAll(ask.selector)[ask.nth];
		if (!panel || !scroller || !target || !scroller.contains(target)) return null;
		const view = scroller.getBoundingClientRect();
		const cover = ask.under === undefined ? null : panel.querySelector(ask.under);
		const top = Math.max(view.top, cover?.getBoundingClientRect().bottom ?? view.top);
		const shift = from - scroller.scrollTop;
		const box = target.getBoundingClientRect();
		return {
			top: box.top - shift,
			bottom: box.bottom - shift,
			height: box.height,
			windowTop: top,
			windowBottom: view.bottom
		};
	}

	/**
	 * Whether the `ask`ed element can be read where the body stands: all of
	 * it, or — taller than the window — the window is filled with it. If not,
	 * which way it lies.
	 */
	function sight(ask: Ask): 'in' | 'below' | 'above' | null {
		if (!scroller) return null;
		const at = measure(ask, scroller.scrollTop);
		if (at === null) return null;
		const whole = at.top >= at.windowTop - 0.5 && at.bottom <= at.windowBottom + 0.5;
		const fills = at.top <= at.windowTop + 0.5 && at.bottom >= at.windowBottom - 0.5;
		if (whole || fills) return 'in';
		return at.top < at.windowTop ? 'above' : 'below';
	}

	/**
	 * Where the body has to be scrolled, from `from`, for the `ask`ed element
	 * to be read: all of it when it fits the window, else from its top.
	 */
	function aim(ask: Ask, from: number): number {
		const at = measure(ask, from);
		if (!panel || !scroller || at === null) return from;
		if (at.top >= at.windowTop - 0.5 && at.bottom <= at.windowBottom + 0.5) return from;
		const room = at.windowBottom - at.windowTop;
		let next: number;
		if (at.top < at.windowTop || at.height > room) {
			next = from + at.top - at.windowTop;
		} else {
			// A little of the body under it, where there is room for that: its
			// edge is not the foot's.
			const air = parseFloat(getComputedStyle(panel).getPropertyValue('--space-lg')) || 0;
			next = from + Math.ceil(at.bottom - at.windowBottom + Math.min(air, room - at.height));
		}
		return Math.min(Math.max(0, next), scroller.scrollHeight - scroller.clientHeight);
	}

	/**
	 * Scroll the body for `asks`, each in turn — the last is where it ends —
	 * as one movement: at once when motion is reduced, else a short smooth
	 * scroll. Only the body moves: the frame, its header and foot do not.
	 */
	function scrollFor(asks: Ask[]): void {
		if (!scroller) return;
		const from = scroller.scrollTop;
		const goal = asks.reduce((at, ask) => aim(ask, at), from);
		if (Math.abs(goal - from) < 0.5) return;
		scroller.scrollTo({ top: goal, behavior: reducedMotion() ? 'instant' : 'smooth' });
	}

	/**
	 * Carry out `asks`, now that nobody's hand is on the body. A person who
	 * has scrolled the body themselves is not taken back from where they
	 * went: what is out of sight is OWED, and the cue says where it is.
	 */
	function bring(asks: Ask[]): void {
		if (!scrolledByHand || revealCue === undefined) {
			scrollFor(asks);
			return;
		}
		const key = (ask: Ask) => `${ask.selector}\n${ask.nth}\n${ask.under ?? ''}`;
		const fresh = asks.filter((ask) => sight(ask) !== 'in' && sight(ask) !== null);
		const kept = owed.filter((ask) => !fresh.some((one) => key(one) === key(ask)));
		owed = [...kept, ...fresh];
		recheckCue();
	}

	/** The cue's own count: what is now in sight is no longer owed. */
	function recheckCue(): void {
		if (owed.length === 0) return;
		const left = owed.filter((ask) => {
			const side = sight(ask);
			return side === 'below' || side === 'above';
		});
		if (left.length === 0 && cueButton !== undefined && document.activeElement === cueButton) {
			panel?.focus({ preventScroll: true });
		}
		if (left.length !== owed.length) owed = left;
		const last = left[left.length - 1];
		if (last !== undefined) cueSide = sight(last) === 'above' ? 'above' : 'below';
	}

	/** The cue was tapped: to what it points, the way a reveal goes. */
	function followCue(): void {
		scrollFor(owed);
	}

	/**
	 * Bring the `nth` `selector` in the body into sight: all of it when it
	 * fits the body's window, else from its top (PR 3 device round — a
	 * verdict that lands below the fold, over a live confirm, is the hole).
	 * `under`: a header the content keeps at the top of the scroll, which
	 * covers what is behind it. Nothing moves when it is already in sight.
	 *
	 * The body is the person's while they use it (owner, 2026-10-28): while
	 * their hand is on it — a finger, a press, the wheel, a scrolling key, its
	 * scrollbar — this waits until they let go and it has settled. Once they
	 * have scrolled it themselves, it is not scrolled for them: a cue
	 * (`revealCue`) says where the new thing is, and a tap on it goes there.
	 * Several asked for together are carried out together, the last asked
	 * for being where the body ends.
	 */
	export function reveal(selector: string, at: { nth?: number; under?: string } = {}): void {
		asked.push({ selector, nth: at.nth ?? 0, under: at.under });
		if (askedSoon) return;
		askedSoon = true;
		queueMicrotask(() => {
			askedSoon = false;
			if (!handOn()) settle();
		});
	}

	/** The body scrolled: by the person while their hand is on it, else by the page. */
	function onBodyScroll(): void {
		if (handOn()) {
			scrolledByHand = true;
			// Still coasting: not settled yet.
			if (!hand.touch && !hand.press && !hand.key) stir();
		}
		recheckCue();
	}

	function onBodyTouch(event: TouchEvent): void {
		hand.touch = event.touches.length > 0;
		if (!hand.touch) stir();
	}

	function onBodyPress(): void {
		hand.press = true;
	}

	function onPressEnd(): void {
		if (!hand.press) return;
		hand.press = false;
		stir();
	}

	/** The keys that scroll what has the focus — not inside a field, where they edit. */
	const SCROLL_KEYS = new Set(['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' ']);

	function onScrollKey(event: KeyboardEvent): void {
		if (!SCROLL_KEYS.has(event.key) || panel === undefined) return;
		const target = event.target instanceof Element ? event.target : null;
		if (!target || !panel.contains(target)) return;
		if (target.closest('input, textarea, select, [contenteditable]')) return;
		// Space on a button presses it.
		if (event.key === ' ' && target.closest('button, a[href], [role="button"]')) return;
		hand.key = true;
	}

	function onScrollKeyUp(): void {
		if (!hand.key) return;
		hand.key = false;
		stir();
	}

	/** Back to its middle: the window changed, or what the sheet holds did. */
	export function recentre(): void {
		held = null;
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
		// The person's hand on the body, for `reveal` (all passive: watched, never claimed).
		node.addEventListener('touchstart', onBodyTouch, { passive: true });
		node.addEventListener('touchend', onBodyTouch, { passive: true });
		node.addEventListener('touchcancel', onBodyTouch, { passive: true });
		node.addEventListener('pointerdown', onBodyPress, { passive: true });
		node.addEventListener('wheel', stir, { passive: true });
		node.addEventListener('scroll', onBodyScroll, { passive: true });
		return () => {
			node.removeEventListener('touchstart', onTouchStart);
			node.removeEventListener('touchmove', onTouchMove);
			node.removeEventListener('touchend', onTouchEnd);
			node.removeEventListener('touchcancel', onTouchEnd);
			node.removeEventListener('touchstart', onBodyTouch);
			node.removeEventListener('touchend', onBodyTouch);
			node.removeEventListener('touchcancel', onBodyTouch);
			node.removeEventListener('pointerdown', onBodyPress);
			node.removeEventListener('wheel', stir);
			node.removeEventListener('scroll', onBodyScroll);
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
		onScrollKey(event);
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
		clearTimeout(quiet);
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

<svelte:window
	onkeydown={onKeydown}
	onkeyup={onScrollKeyUp}
	onpointerup={onPressEnd}
	onpointercancel={onPressEnd}
	onblur={() => {
		onPressEnd();
		onScrollKeyUp();
	}}
	onresize={() => {
		recentre();
		recheckCue();
	}}
/>

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
		class:footed={footer !== undefined}
		style:transform={offset !== 0 || settling !== 'none' ? `translateY(${offset}px)` : undefined}
		style:top={held ? 'auto' : undefined}
		style:bottom={held ? `${held.bottom}px` : undefined}
		style:max-height={held ? `${held.most}px` : undefined}
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
			<!-- A div, not a <header>: a sheet is a dialog, and its title row is
			     no second banner beside the page's own. -->
			<div class="head" data-sheet-grip>
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
			</div>
		{/if}
		<div class="content" bind:this={scroller}>
			{@render children()}
		</div>
		<!-- `reveal`'s cue: over the body's bottom edge, a box of no height,
		     so the body, the foot and the confirm in it are where they were. -->
		<div class="cue-edge" aria-live="polite">
			{#if owed.length > 0 && revealCue}
				<button
					type="button"
					class="cue"
					data-testid="sheet-reveal-cue"
					data-side={cueSide}
					bind:this={cueButton}
					onclick={followCue}
				>
					{cueSide === 'above' ? revealCue.above : revealCue.below}
				</button>
			{/if}
		</div>
		{#if footer}
			<div class="foot">{@render footer()}</div>
		{/if}
	</div>
</div>

<style>
	/* --- layers ------------------------------------------------------------ */

	/* `wallet` and `menu` sit within the nearest positioned ancestor (the
	   phone frame, the page) — never taller than the window shows (§19.10).
	   A sheet nothing frames (the multisig host, the account switcher) is
	   placed in the initial containing block, which iOS Safari sizes to the
	   LARGE viewport, the one with its toolbar collapsed: the sheet's foot —
	   the Create multisig sheet's Next — stood under Safari 26's floating
	   toolbar until a scroll collapsed it. The dynamic viewport (`dvh`) is
	   what is seen now, toolbar and all; in a frame shorter than the window
	   the cap never binds. The box takes no taps: the scrim and the sheet do. */
	.root.wallet,
	.root.menu {
		position: absolute;
		inset-inline: 0;
		top: 0;
		height: 100%;
		max-height: 100dvh;
		pointer-events: none;
	}

	.root.wallet > *,
	.root.menu > * {
		pointer-events: auto;
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
	.head {
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

	.head {
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

	/* Their gutter is the content's, not the sheet's: a title row drawn on one
	   of them (the page hand-off's prompt) stands on the same column, its ✕
	   reaching into the gutter as the wallet skin's does — phone and card. */
	.menu .head,
	.signing .head,
	.prompt .head {
		padding-inline: var(--layout-screenPaddingX);
	}

	/* A foot outside the scroll (the signing sheet's confirm): the gutter and
	   the bottom space are ITS, and the body ends where its content does — so
	   a sheet that fits is drawn exactly as when the foot was the last thing
	   in its content. */
	.foot {
		flex: none;
	}

	/* The cue stands on the body's bottom edge, over what scrolls — never
	   over the foot, and taking no room from either. */
	.cue-edge {
		position: relative;
		flex: none;
		height: 0;
		display: flex;
		justify-content: center;
	}

	.cue {
		position: absolute;
		bottom: var(--space-md);
		z-index: 2;
		max-width: calc(100% - 2 * var(--layout-screenPaddingX));
		min-height: var(--size-control-sm);
		padding-inline: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-full);
		background: var(--color-bg-base);
		color: var(--color-fg-base);
		box-shadow: var(--shadow-md);
		font: inherit;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		cursor: pointer;
	}

	.menu .foot,
	.signing .foot,
	.prompt .foot {
		padding-inline: var(--layout-screenPaddingX);
		padding-bottom: calc(var(--space-3xl) + env(safe-area-inset-bottom, 0%));
	}

	.menu .sheet.footed > .content,
	.signing .sheet.footed > .content,
	.prompt .sheet.footed > .content {
		padding-bottom: 0;
	}

	/* A hairline where the body goes on under the foot, and none where it
	   ends: the first layer travels with the content and covers the second,
	   which stays at the body's bottom edge. No box, so nothing moves. */
	.wallet .sheet.footed > .content {
		background:
			linear-gradient(var(--color-bg-base), var(--color-bg-base)) local bottom / 100%
				var(--space-sm) no-repeat,
			linear-gradient(var(--color-border-base), var(--color-border-base)) scroll bottom / 100%
				var(--border-hairline) no-repeat;
	}

	.signing .sheet.footed > .content {
		background:
			linear-gradient(var(--color-bg-raised), var(--color-bg-raised)) local bottom / 100%
				var(--space-sm) no-repeat,
			linear-gradient(var(--color-border-base), var(--color-border-base)) scroll bottom / 100%
				var(--border-hairline) no-repeat;
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
