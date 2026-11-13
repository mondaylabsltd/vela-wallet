<script lang="ts">
	/**
	 * A logo from the chain-data endpoint, drawn OVER whatever the parent
	 * draws beneath it — a three-letter glyph, a letter on a colour, a dot.
	 * The parent's mark is what shows until the bytes arrive and what stays
	 * when they never do (the endpoint has no logo for this chain, the person
	 * pointed 服务端点 at a mirror without one, the network is down).
	 *
	 * Candidates are tried in order; a failure is remembered for as long as the
	 * core says a miss lasts (`logo-cache`, 60 s for an image that gives no
	 * status — spec 082 RE10), so the second row that asks for the same missing
	 * logo falls back without a request, and the logo is tried again after it.
	 * The parent must be `position: relative`.
	 */
	import { onDestroy } from 'svelte';
	import { hasFailed, markFailed, probeMiss } from '$lib/services/logo-cache';

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

	let retry: ReturnType<typeof setTimeout> | undefined;
	let gone = false;
	function schedule(ttl: number | null) {
		clearTimeout(retry);
		// Try it again once its miss has run out (a network blip is not a
		// missing logo). `null`: the miss lasts the session.
		if (ttl !== null && !gone) retry = setTimeout(() => (failures += 1), ttl + 1);
	}
	function fail(url: string) {
		schedule(markFailed(url));
		failures += 1;
		// home-20: then learn what the miss was. A page in place of a logo, or
		// a 404, is no logo for the session — not a request every minute (the
		// retry already set only re-reads the cache, which now says no).
		void probeMiss(url).then((ttl) => {
			if (ttl !== null) schedule(ttl);
		});
	}
	onDestroy(() => {
		gone = true;
		clearTimeout(retry);
	});

	// Invisible until its bytes are in. The element has an opaque background (a
	// transparent PNG must not show the letter through it), so a logo that is
	// still loading — or hanging on a host that never answers — used to blank
	// out the very fallback it sits over (founder-adjacent, 2026-09-19: a site
	// with no icon drew an empty disc instead of its initial).
	let loaded = $state<string | undefined>();
	// The element says which bytes arrived (ob-16): a logo that lands after its
	// row has gone (a sheet closed mid-load) read `src`, a derived of an effect
	// already destroyed — Svelte's `derived_inert` warning, on every such row.
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
		onload={(event) => (loaded = event.currentTarget.getAttribute('src') ?? undefined)}
		onerror={(event) => fail(event.currentTarget.getAttribute('src') ?? '')}
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
