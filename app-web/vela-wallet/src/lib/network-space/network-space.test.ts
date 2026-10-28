/**
 * perf-1 / perf-2: the spaces' store and layer sit above every page, Welcome
 * included, and Welcome loads neither the core's wasm nor the dApp channel
 * (`budgets.e2e.ts`). Booting the store waits for a core somebody else loaded;
 * the layer reaches the intents only when the person answers the switch.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it, vi } from 'vitest';

const core = vi.hoisted(() => {
	let loaded!: () => void;
	const whenLoaded = new Promise<void>((resolve) => (loaded = resolve));
	return {
		loaded: () => loaded(),
		loadCore: vi.fn(async () => undefined),
		whenCoreLoaded: vi.fn(() => whenLoaded),
		networkSpaceRead: vi.fn((stored?: string) =>
			stored?.trim() === 'testnet' ? 'testnet' : 'mainnet'
		)
	};
});
vi.mock('$app/environment', () => ({ browser: false }));
vi.mock('$lib/core/client', () => core);

const { networkSpace } = await import('./network-space.svelte');

describe('networkSpace.boot', () => {
	it('never loads the core, and reads the space through it once it is here', async () => {
		let booted = false;
		void networkSpace.boot().then(() => (booted = true));
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(booted).toBe(false);
		expect(core.networkSpaceRead).not.toHaveBeenCalled();

		core.loaded();
		await networkSpace.boot();
		expect(booted).toBe(true);
		expect(core.networkSpaceRead).toHaveBeenCalledTimes(1);
		expect(core.loadCore).not.toHaveBeenCalled();
		expect(networkSpace.current).toBe('mainnet');
	});
});

describe('NetworkSpaceLayer', () => {
	it('imports the intents only inside the switch’s answer', () => {
		const source = readFileSync(join(import.meta.dirname, 'NetworkSpaceLayer.svelte'), 'utf8');
		expect(source).not.toMatch(/^\s*import\s[^;]*\$lib\/intents/m);
		expect(source).toContain("import('$lib/intents/index.svelte')");
	});
});
