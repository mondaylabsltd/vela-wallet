/**
 * `vela_advise` and `vela_watch_reviews` as the bridge runs them (spec 103):
 * the real core (shipped wasm) over the signing request on the sheet. An
 * opinion lands on the sheet bound to the digest it was given for; one for
 * a wallet the person did not allow lands nowhere.
 */
import { readFileSync } from 'node:fs';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { initSync, intentAdvice, intentReview } from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';

const ME = '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const live = {
	request: null as Record<string, unknown> | null,
	wallets: [ME] as string[]
};

vi.mock('$lib/core/client', () => ({ intentAdvice, intentReview }));
vi.mock('$lib/session/core/session.svelte', () => ({ session: { view: { address: ME } } }));
vi.mock('$lib/multisig/core/store.svelte', () => ({ multisig: { active: null } }));
vi.mock('$lib/services/endpoints', () => ({ getEthereumDataURL: () => 'https://data.example' }));
vi.mock('$lib/settings/core/network-admin.svelte', () => ({
	networkAdmin: { view: { loaded: false } }
}));
vi.mock('$lib/signing/core/sheet.svelte', () => ({
	signingSheet: { clear: null, guard: null }
}));
vi.mock('$lib/signing/core/sign-resident.svelte', () => ({
	signRequest: {
		get view() {
			return { request: live.request };
		}
	}
}));
vi.mock('$lib/intents/index.svelte', () => ({
	world: () => ({
		session: { address: ME, accounts: [] },
		contacts: null,
		currency: null,
		feeTier: null,
		preferences: {},
		agent: { enabled: true, wallets: live.wallets },
		operations: [],
		balance: null,
		feed: null,
		networks: null,
		networkSpace: 'mainnet',
		multisigs: [],
		shownMultisig: null
	})
}));

const { advise, watch } = await import('./advisor.svelte');
const { openReview, sheetBeside } = await import('./advice.svelte');

beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

function request(id: string, text: string) {
	return {
		id,
		method: 'personal_sign',
		kind: 'personal_sign',
		params_json: JSON.stringify([text, ME]),
		origin: 'https://site.example',
		dapp: null,
		chain_id: 1,
		signer_address: ME,
		first_party: false
	};
}

type Outcome = { isError?: boolean; structuredContent: Record<string, unknown> };

beforeEach(() => {
	live.request = request('r1', 'Sign in');
	live.wallets = [ME];
	sheetBeside.publish({ requestId: null, sim: null, simPending: false, fee: null });
});

describe('vela_advise', () => {
	it('lands on the sheet, current, for the digest on screen', () => {
		const digest = openReview()!.review_digest;
		const out = advise({ review_digest: digest, verdict: 'caution', summary: 'Sign-in only.' });
		expect((out as Outcome).structuredContent).toMatchObject({
			status: 'completed',
			shown_as: 'current'
		});
		expect(sheetBeside.advice).toMatchObject({
			requestId: 'r1',
			advice: { verdict: 'caution', summary: 'Sign-in only.' }
		});
	});

	it('for an earlier version, is kept but never as current', () => {
		const digest = openReview()!.review_digest;
		live.request = request('r1', 'Sign in, and something else');
		expect(openReview()!.review_digest).not.toBe(digest);
		const out = advise({ review_digest: digest, verdict: 'looks_fine', summary: 'Fine.' });
		expect((out as Outcome).structuredContent).toMatchObject({ shown_as: 'earlier_version' });
	});

	it('is refused for a wallet the person did not allow, and nothing lands', () => {
		live.wallets = [];
		const digest = openReview()!.review_digest;
		const out = advise({ review_digest: digest, verdict: 'danger', summary: 'Stop.' }) as Outcome;
		expect(out.isError).toBe(true);
		expect(out.structuredContent).toMatchObject({ reason: { code: 'wallet_not_allowed' } });
		expect(sheetBeside.advice).toBeNull();
	});

	it('is refused when no review is open', () => {
		live.request = null;
		const out = advise({ review_digest: '0x1', verdict: 'unsure', summary: 'Hm.' }) as Outcome;
		expect(out.structuredContent).toMatchObject({ reason: { code: 'not_found' } });
	});

	it('goes with its request', () => {
		const digest = openReview()!.review_digest;
		advise({ review_digest: digest, verdict: 'caution', summary: 'Check.' });
		sheetBeside.publish({ requestId: 'r2', sim: null, simPending: false, fee: null });
		expect(sheetBeside.advice).toBeNull();
	});
});

describe('vela_watch_reviews', () => {
	it('answers the review on screen with its digest', async () => {
		const out = (await watch({})) as { review_digest: string; timed_out: boolean };
		expect(out.timed_out).toBe(false);
		expect(out.review_digest).toBe(openReview()!.review_digest);
	});

	it('answers a new review once it opens', async () => {
		const seen = openReview()!.review_digest;
		setTimeout(() => (live.request = request('r2', 'Another')), 300);
		const out = (await watch({ since_digest: seen, timeout_ms: 5000 })) as {
			review_digest: string;
			timed_out: boolean;
		};
		expect(out.timed_out).toBe(false);
		expect(out.review_digest).not.toBe(seen);
	});

	it('times out cleanly', async () => {
		const seen = openReview()!.review_digest;
		const out = await watch({ since_digest: seen, timeout_ms: 300 });
		expect(out).toEqual({ timed_out: true, review_digest: seen, review: null });
	});

	it('is refused for a wallet the person did not allow', async () => {
		live.wallets = [];
		const out = (await watch({})) as Outcome;
		expect(out.structuredContent).toMatchObject({ reason: { code: 'wallet_not_allowed' } });
	});
});
