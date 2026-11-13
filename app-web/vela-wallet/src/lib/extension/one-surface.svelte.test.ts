/**
 * A request holds the side panel live from an EFFECT (`DappRequestHost`), in
 * a real Svelte runtime. The first cut counted the hold inside that effect
 * and so read what it wrote: the effect ran itself again until Svelte gave
 * up, and the request window stood on "Preparing wallet…" for good.
 */
import { flushSync } from 'svelte';
import { describe, expect, it } from 'vitest';
import { OneSurface } from './one-surface.svelte';

describe('a request holding the side panel from an effect', () => {
	it('takes one hold, releases it once, and never runs itself again', () => {
		const surface = new OneSurface({});
		let busy = $state(false);
		let runs = 0;
		const stop = $effect.root(() => {
			$effect(() => {
				runs += 1;
				if (!busy) return;
				return surface.hold();
			});
		});
		flushSync();
		busy = true;
		flushSync();
		busy = false;
		flushSync();
		expect(runs).toBe(3);
		stop();
	});
});
