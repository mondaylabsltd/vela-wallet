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

	// Invisible until its bytes are in. The element has an opaque background (a
	// transparent PNG must not show the letter through it), so a logo that is
	// still loading — or hanging on a host that never answers — used to blank
	// out the very fallback it sits over (founder-adjacent, 2026-09-19: a site
	// with no icon drew an empty disc instead of its initial).
	let loaded = $state<string | undefined>();
</script>

{#if src !== undefined}
	<img
		class="logo"
		class:ready={loaded === src}
		{src}
		alt=""
		loading="lazy"
		decoding="async"
		referrerpolicy="no-referrer"
		draggable="false"
		onload={() => (loaded = src)}
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
		opacity: 0;
	}

	.logo.ready {
		opacity: 1;
	}
</style>
