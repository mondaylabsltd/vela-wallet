/**
 * Facts the suites ask of the CORE instead of restating.
 *
 * A test that says "the default speed is Fast" keeps saying so after the core
 * changed its mind — five did, for days, once the factory default became
 * Standard. The shipped core is loaded here in Node (the same artifact the
 * page fetches, read off disk) and asked: so a suite about what happens AT
 * the default never has to know which one it is.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const REPO_ROOT = join(import.meta.dirname, '..', '..', '..');
const PKG = join(REPO_ROOT, 'rust', 'pkg-web');

/** One of the core's state machines, as the wasm glue exposes it. */
interface Machine {
	view(): string;
	free(): void;
}
type ShippedCore = Record<string, new () => Machine>;

let shipped: Promise<ShippedCore> | undefined;

/** The committed wasm artifact, initialised once per worker. */
function shippedCore(): Promise<ShippedCore> {
	shipped ??= (async () => {
		const glue = await import(pathToFileURL(join(PKG, 'vela_core.js')).href);
		const { WASM_URL } = await import(pathToFileURL(join(PKG, 'vela_core_wasm_url.js')).href);
		glue.initSync({
			module: readFileSync(join(REPO_ROOT, 'assets', 'wasm', String(WASM_URL).replace(/^\//, '')))
		});
		return glue as ShippedCore;
	})();
	return shipped;
}

export type Speed = 'fast' | 'standard' | 'slow';

/**
 * What the `fee_tier_pref` machine says before anybody has chosen anything:
 * the speed every send starts at on a fresh device (its
 * `FACTORY_DEFAULT`), and the speeds it offers, fastest first.
 */
export async function speedDefaults(): Promise<{ factory: Speed; offered: Speed[] }> {
	const core = await shippedCore();
	const machine = new core.FeeTierPrefCore();
	try {
		const view = JSON.parse(machine.view()) as {
			tier: Speed;
			committed: boolean;
			offered: Speed[];
		};
		if (view.committed) throw new Error('a fresh fee_tier_pref machine says a speed was chosen');
		return { factory: view.tier, offered: view.offered };
	} finally {
		machine.free();
	}
}
