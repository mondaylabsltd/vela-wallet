/**
 * "This site isn't connected" must not outlive the connection (device walk of
 * 2026-10-11, item 7).
 *
 * On Android a page that asked for a signature before it connected left
 * 「此网站尚未连接 · personal_sign」 on its tab for as long as the tab lived,
 * connected or not: the in-app browsers keep a record of each tab's last
 * failure, and the banner was drawn from it. The core now stops counting that
 * failure once the tab's site is connected.
 *
 * The web has no in-app browser, no tab record and no banner. What it has is
 * the extension's request surface, and this proves that surface keeps nothing
 * to go stale:
 *
 *   - "not connected" is the core's answer to ONE request, asked of what is in
 *     storage at that moment (`evaluate`); the same request asked again after
 *     the site connects is answered differently, and nothing in between
 *     remembers the first;
 *   - the only place a refusal reaches a person's eyes is the request WINDOW
 *     of the request that was refused — a document the worker closes, or
 *     replaces whole, when the site asks again (spec 094 S7); the side panel
 *     draws no refusal at all.
 *
 * `e2e/extension-signing.e2e.ts` walks the same thing in the packaged
 * extension.
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DAppGrant } from './grants';

const ALICE = `0x${'a1'.repeat(20)}`;
const SITE = 'https://arcorbs.game3.app';
const MESSAGE = '0x68656c6c6f';

const stored = new Map<string, DAppGrant>();
const answers: unknown[] = [];

vi.mock('$lib/core/client', async (real) => ({
	...(await real<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));
vi.mock('./grants', () => ({
	getGrant: async (origin: string) => stored.get(origin) ?? null,
	setGrant: async (grant: DAppGrant) => void stored.set(grant.origin, grant)
}));
vi.mock('./transport', () => ({
	answerRequest: async (_rid: string, answer: unknown) => {
		answers.push(answer);
		return true;
	}
}));

const { evaluate } = await import('./request');

let asked = 0;
const sign = () =>
	evaluate(
		{
			rid: `7:${(asked += 1)}`,
			id: String(asked),
			method: 'personal_sign',
			params: [MESSAGE, ALICE],
			origin: SITE,
			tabId: 7,
			at: 0
		},
		{ activeAddress: ALICE, addresses: [ALICE] }
	);

beforeEach(() => {
	stored.clear();
	answers.length = 0;
});

describe('"not connected" is the answer to one request, about now', () => {
	it('a signature asked before the site connects is refused — to the site, at once', async () => {
		expect(await sign()).toEqual({
			kind: 'refused',
			code: 4100,
			message: 'Connect Vela Wallet to this site first'
		});
		expect(answers).toEqual([
			{ error: { code: 4100, message: 'Connect Vela Wallet to this site first' } }
		]);
	});

	it('once the site is connected the same request is taken up: nothing remembers the refusal', async () => {
		expect((await sign()).kind).toBe('refused');
		// The person connects the site.
		stored.set(SITE, { origin: SITE, address: ALICE, chainId: 100, grantedAt: 1 });
		expect(await sign()).toMatchObject({ kind: 'signing' });
		expect(await sign()).toMatchObject({ kind: 'signing' });
		// Only the first was ever refused.
		expect(answers).toHaveLength(1);
	});

	it('and it is true again when the site is disconnected', async () => {
		stored.set(SITE, { origin: SITE, address: ALICE, chainId: 100, grantedAt: 1 });
		expect((await sign()).kind).toBe('signing');
		stored.delete(SITE);
		expect((await sign()).kind).toBe('refused');
	});
});

describe('where a refusal can be read', () => {
	const host = readFileSync('src/lib/dapp/DappRequestHost.svelte', 'utf8');
	const markup = host.slice(host.indexOf('{#if mode ==='), host.indexOf('<style>'));
	const [panel, windowed] = markup.split('{:else if showWindowChrome}');

	it('the side panel — which outlives its requests — draws no refusal', () => {
		expect(panel).toContain("{#if mode === 'panel'}");
		expect(panel).not.toContain('refused');
		expect(panel).not.toContain('stage.message');
	});

	it('the request window draws its own request’s, and a request’s stage is replaced whole by the next', () => {
		expect(windowed.match(/stage\.kind === 'refused'/g)).toHaveLength(1);
		expect(windowed).toContain('{stage.message}');
		// Every request taken sets the stage from the core's answer to IT.
		expect(host).toContain('stage = await evaluate(incoming, facts);');
	});

	it('no other surface of the web or the extension says a site is not connected', () => {
		// The corpus's banner words (`componentsUi.browserStatus.reason.
		// notConnected`) belong to the in-app browsers' tab status; the web
		// resolves none of that block's reasons but the four a signing sheet
		// and the add-network sheet use.
		const engine = readFileSync('src/lib/i18n/engine.server.ts', 'utf8');
		expect(engine).not.toContain('browserStatus.reason.notConnected');
		const reasons = [...engine.matchAll(/browserStatus\.reason\.(\w+)/g)].map((m) => m[1]).sort();
		expect(reasons).toEqual([
			'badRpc',
			'signerFailed',
			'signerNotDiscoverable',
			'signerUnavailable'
		]);
	});
});
