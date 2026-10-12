/**
 * Settings does not drop when the keys block loads (device walk, item 13.1) —
 * on the real page, with every chain held back.
 *
 * The block was one grey row until the registry answered, and then everything
 * under it moved by the rest of the wallet's keys. The component suite
 * (`keys-steady.svelte.test.ts`) measures the two screens through every
 * answer; what it cannot see is the page's own wiring — that the rows really
 * are there before any chain has said anything. Here no request leaves the
 * browser until the test lets it, so what is on screen is the device's record
 * and nothing else; then every chain is refused, which is the one ending a
 * test can give without a registry: the rows stay, and the registry's one
 * sentence is added under them.
 */
import { expect, test, type Page, type Route } from '@playwright/test';
import { en, seedSignedIn } from './live-helpers';

const KEY = (n: number) => '04' + String(n).repeat(128);
const NAMES = ['MacBook Pro', 'YubiKey 5C', 'Pixel 9'];

/** A three-key record, the second key the one this device signs with. */
const THREE_KEYS = {
	id: 'c1',
	name: 'E2E Wallet',
	address: '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e',
	public_key_hex: KEY(1),
	created_at_iso: '2026-01-01T00:00:00.000Z',
	keys: NAMES.map((name, index) => ({
		credential_id: `c${index + 1}`,
		public_key_hex: KEY(index + 1),
		name,
		transports: ''
	})),
	sign_in_key: { credential_id: 'c2', method: 'platform' }
};

/**
 * Every request that is not the app's own is HELD: nothing is answered until
 * `refuse()`, after which each one fails at once. While held, no chain has
 * said a word — whatever the page shows, it shows from the device.
 */
async function holdChains(page: Page) {
	const held: Route[] = [];
	let refusing = false;
	await page.route(/^https?:\/\/(?!localhost|127\.0\.0\.1)/, (route) => {
		if (refusing) return route.abort('failed');
		held.push(route);
	});
	return {
		asked: () => held.length,
		async refuse() {
			refusing = true;
			await Promise.all(held.splice(0).map((route) => route.abort('failed').catch(() => {})));
		}
	};
}

interface GreyFrames {
	__greyFrames: number;
}

/**
 * Counts the FRAMES in which the block is its one grey row. Asked in an
 * animation frame — what is in the document then is what gets painted — so a
 * row that is swapped for the real ones before the browser draws is not
 * counted, and one a person could see is.
 */
async function countGreyFrames(page: Page): Promise<void> {
	await page.addInitScript(() => {
		const state = window as unknown as GreyFrames;
		state.__greyFrames = 0;
		const frame = () => {
			if (document.querySelector('section.keys .row.skeleton') !== null) state.__greyFrames += 1;
			requestAnimationFrame(frame);
		};
		requestAnimationFrame(frame);
	});
}

interface Reading {
	block: number;
	under: number;
	rows: number[];
	copy: number;
	count: string | null;
	names: string[];
	said: string;
}

/**
 * The page's own typefaces have arrived: nothing is still loading, and the
 * text face is in.
 *
 * Until they are, the words stand in the fallback's metrics — each key's row
 * a pixel shorter, the block four higher — and a reading taken then is not
 * where anything rests. On an idle machine the first reading sometimes was
 * (3 runs in 80): rows read at 235 / 321 / 407 that stand at 239 / 326 / 413,
 * each time with `document.fonts.status` still "loading".
 */
async function typefacesIn(page: Page): Promise<void> {
	await expect
		.poll(() =>
			page.evaluate(
				() =>
					document.fonts.status === 'loaded' &&
					[...document.fonts].every((face) => face.status !== 'loading') &&
					[...document.fonts].some(
						(face) => face.status === 'loaded' && face.family.includes('Plus Jakarta Sans')
					)
			)
		)
		.toBe(true);
}

/** The block's geometry and words, read in the page — once its typefaces are in. */
async function read(page: Page, wide: boolean): Promise<Reading> {
	await typefacesIn(page);
	return page.evaluate((wide) => {
		const round = (n: number) => Math.round(n * 100) / 100;
		const block = document.querySelector('section.keys') as HTMLElement;
		const outer = wide ? block : (block.closest('.keys-block') as HTMLElement);
		const under = outer.nextElementSibling as HTMLElement;
		const top = (element: Element) => round(element.getBoundingClientRect().top + window.scrollY);
		return {
			block: round(block.getBoundingClientRect().height),
			under: top(under),
			rows: [...block.querySelectorAll('li.key, .row.skeleton')].map(top),
			copy: top(block.querySelector('button.backup') as HTMLElement),
			count: block.querySelector('h2 .count')?.textContent ?? null,
			names: [...block.querySelectorAll('li.key .name')].map((n) => n.textContent?.trim() ?? ''),
			said: block.innerText
		};
	}, wide);
}

const LAYOUTS = [
	{ layout: 'phone', viewport: { width: 390, height: 844 }, wide: false },
	{ layout: 'wide', viewport: { width: 1440, height: 900 }, wide: true }
] as const;

for (const { layout, viewport, wide } of LAYOUTS) {
	test.describe(`${layout} ${viewport.width}x${viewport.height}`, () => {
		test.use({ viewport });

		for (const count of [1, 3] as const) {
			test(`${count} key(s): the rows stand before any chain answers, and stay when none does`, async ({
				page
			}) => {
				const chains = await holdChains(page);
				await countGreyFrames(page);
				if (count === 3) {
					await page.addInitScript((account) => {
						window.localStorage.setItem('vela.accounts', JSON.stringify([account]));
						window.localStorage.setItem('vela.activeAccountIndex', '0');
					}, THREE_KEYS);
				}
				await seedSignedIn(page);
				await page.goto('/en/settings');
				await expect(page.getByText('E2E Wallet').first()).toBeVisible();
				// The copy's row is asking — so the page has met its account, and
				// both walks are out with nothing come back.
				const block = page.locator('section.keys');
				await expect(block.locator('button.backup .state')).toHaveText(
					en('componentsUi.funding.checking')
				);
				await expect.poll(() => chains.asked()).toBeGreaterThan(0);

				const asking = await read(page, wide);
				console.log(
					`KEYS-STILL ${JSON.stringify({ layout, keys: count, when: 'asking', ...asking })}`
				);

				// What the device's record knows is on screen: every key, the count.
				expect(asking.rows, 'a row for every key').toHaveLength(count);
				expect(asking.count).toBe(String(count));
				expect(asking.names).toEqual(count === 1 ? ['E2E Wallet'] : NAMES);
				// …from the first frame the block was drawn in: the one grey row
				// that used to stand for all of them was never painted.
				await expect(block.locator('.row.skeleton')).toHaveCount(0);
				expect(await page.evaluate(() => (window as unknown as GreyFrames).__greyFrames)).toBe(0);
				// …and nothing the registry may contradict: no word for where a key
				// lives, no badge it would have to vouch for, no "couldn't reach".
				for (const guess of [
					'onboarding.create.providerPlatform',
					'onboarding.create.providerGeneric',
					'onboarding.create.providerSecurityKey',
					'onboarding.create.keySyncedBadge',
					'settingsModals.keys.notSynced',
					'settingsModals.keys.userVerified',
					'settingsModals.keys.fromDevice'
				]) {
					expect(asking.said, guess).not.toContain(en(guess));
				}
				await expect(block.locator('li.key .mark-slot > *')).toHaveCount(0);
				if (count === 3) {
					await expect(block.locator('li.key').nth(1).locator('.pill:not(.quiet)')).toHaveText([
						en('settingsModals.keys.signsHere')
					]);
				}

				// No chain answers at all: the registry's one sentence is added
				// under rows that stay where they stood.
				await chains.refuse();
				const note = block.locator('p.note').first();
				await expect(note).toHaveText(en('settingsModals.keys.fromDevice'), { timeout: 60_000 });
				await expect(block.locator('button.backup .state')).toHaveText(
					en('settingsModals.backup.couldNotCheck'),
					{ timeout: 60_000 }
				);
				const silent = await read(page, wide);
				const added = Math.round(((await note.boundingBox())?.height ?? 0) * 100) / 100;
				console.log(
					`KEYS-STILL ${JSON.stringify({ layout, keys: count, when: 'silent', note: added, ...silent })}`
				);
				expect(silent.rows, 'each key’s row where it stood').toEqual(asking.rows);
				expect(silent.names).toEqual(asking.names);
				expect(added).toBeGreaterThan(0);
				expect(silent.copy - asking.copy, 'the copy’s row, down by the sentence').toBeCloseTo(
					added,
					1
				);
				expect(silent.under - asking.under, 'and what is under the block').toBeCloseTo(added, 1);
				expect(silent.block - asking.block).toBeCloseTo(added, 1);
				// Now the device's own reading of where a key lives is all there is, and is said.
				expect(silent.said).toContain(en('onboarding.create.providerPlatform'));
			});
		}
	});
}
