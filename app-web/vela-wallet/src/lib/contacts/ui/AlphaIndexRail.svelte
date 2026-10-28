<script lang="ts">
	interface Props {
		/** Full A–Z + # alphabet (research.md D4) — always rendered whole. */
		letters: string[];
		/** Letters that actually have a section; others jump to the nearest one. */
		available: string[];
		/** Static gallery variant: bubble HUD shown without a pointer. */
		bubble?: string;
		onjump?: (letter: string) => void;
	}

	let { letters, available, bubble, onjump }: Props = $props();

	let rail = $state<HTMLDivElement | undefined>();
	let active = $state<string | undefined>(undefined);
	let bubbleY = $state(0);

	const shown = $derived(active ?? bubble);

	/** Nearest existing section for a letter the fixture has no rows under. */
	function resolve(letter: string): string | undefined {
		if (available.includes(letter)) return letter;
		if (available.length === 0) return undefined;
		const index = letters.indexOf(letter);
		let best = available[0];
		let bestDistance = Number.POSITIVE_INFINITY;
		for (const candidate of available) {
			const distance = Math.abs(letters.indexOf(candidate) - index);
			if (distance < bestDistance) {
				bestDistance = distance;
				best = candidate;
			}
		}
		return best;
	}

	function letterAt(clientY: number): { letter: string; y: number } | undefined {
		if (rail === undefined) return undefined;
		const box = rail.getBoundingClientRect();
		const ratio = (clientY - box.top) / box.height;
		const index = Math.min(letters.length - 1, Math.max(0, Math.floor(ratio * letters.length)));
		return { letter: letters[index], y: clientY - box.top };
	}

	function track(event: PointerEvent) {
		const hit = letterAt(event.clientY);
		if (hit === undefined || hit.letter === active) {
			if (hit !== undefined) bubbleY = hit.y;
			return;
		}
		active = hit.letter;
		bubbleY = hit.y;
		const target = resolve(hit.letter);
		if (target !== undefined) onjump?.(target);
	}

	function onpointerdown(event: PointerEvent) {
		rail?.setPointerCapture(event.pointerId);
		track(event);
	}

	function onpointermove(event: PointerEvent) {
		if (rail?.hasPointerCapture(event.pointerId) !== true) return;
		track(event);
	}

	function onpointerup(event: PointerEvent) {
		if (rail?.hasPointerCapture(event.pointerId) === true)
			rail.releasePointerCapture(event.pointerId);
		active = undefined;
	}
</script>

<div
	class="rail"
	data-tap
	bind:this={rail}
	role="presentation"
	{onpointerdown}
	{onpointermove}
	{onpointerup}
	onpointercancel={onpointerup}
>
	{#each letters as letter (letter)}
		<span class="letter" class:on={available.includes(letter)}>{letter}</span>
	{/each}
	<span class="bubble" class:show={shown !== undefined} style:top="{bubbleY}px" aria-hidden="true">
		{shown ?? ''}
	</span>
</div>

<style>
	.rail {
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: space-between;
		width: var(--space-3xl);
		flex-shrink: 0;
		touch-action: none;
		user-select: none;
		cursor: pointer;
	}

	.letter {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-subtle);
		line-height: var(--leading-tight);
		opacity: var(--opacity-dim);
	}

	.letter.on {
		color: var(--color-fg-muted);
		opacity: 1;
	}

	.bubble {
		position: absolute;
		inset-inline-end: 100%;
		margin-inline-end: var(--space-md);
		transform: translateY(-50%);
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-emptyStateCircle);
		height: var(--size-emptyStateCircle);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		box-shadow: var(--shadow-lg);
		color: var(--color-fg-base);
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		pointer-events: none;
		opacity: 0;
		/* fade-out 80ms; fade-in 120ms below (SPEC 手机 · 索引条) */
		transition: opacity var(--motion-bubble-out) ease-out;
	}

	.bubble.show {
		opacity: 1;
		transition-duration: var(--motion-bubble-in);
	}

	/* Reduced motion: no bubble animation, jump stays direct (SPEC 手机). */
	@media (prefers-reduced-motion: reduce) {
		.bubble {
			transition: none;
		}
	}
</style>
