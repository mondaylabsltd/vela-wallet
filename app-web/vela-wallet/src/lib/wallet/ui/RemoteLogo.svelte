<script lang="ts">
	/**
	 * A logo from the chain-data endpoint, drawn OVER whatever the parent
	 * draws beneath it — a three-letter glyph, a letter on a colour, a dot.
	 * The parent's mark is what shows until the bytes arrive and what stays
	 * when they never do (the endpoint has no logo for this chain, the person
	 * pointed 服务端点 at a mirror without one, the network is down).
	 *
	 * Candidates are tried in order; a failure is remembered for the session
	 * (`logo-cache`), so the second row that asks for the same missing logo
	 * falls back without a request. The parent must be `position: relative`.
	 */
	import { hasFailed, markFailed } from '$lib/services/logo-cache';

	interface Props {
		urls?: string[];
	}

	let { urls }: Props = $props();

	// Bumped on every failure so the candidate list re-derives past it; the
	// set itself is module state shared by every mark on the page.
	let failures = $state(0);
	const src = $derived.by(() => {
		void failures;
		return (urls ?? []).find((url) => !hasFailed(url));
	});

	function fail(url: string) {
		markFailed(url);
		failures += 1;
	}
</script>

{#if src !== undefined}
	<img
		class="logo"
		{src}
		alt=""
		loading="lazy"
		decoding="async"
		referrerpolicy="no-referrer"
		draggable="false"
		onerror={() => fail(src)}
	/>
{/if}

<style>
	.logo {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		border-radius: var(--radius-full);
		object-fit: cover;
		background: var(--color-bg-base);
	}
</style>
