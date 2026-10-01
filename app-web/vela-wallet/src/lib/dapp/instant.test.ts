/**
 * The two twins, pinned against the machine that owns them (spec 027 T335).
 *
 * The service worker cannot run the core — loading a 3.6 MB binary to answer
 * `eth_accounts` on every page load is not a trade anyone would make — so two
 * of `dapp_permissions`' rules exist a second time in `extension/lib/protocol.js`.
 * A rule that LOOKS like the source of truth while something else quietly
 * re-implements it is worse than no rule, because the next edit lands on the
 * copy nobody runs. So these drive the REAL core over the same inputs and
 * demand the same answers.
 */
// The same one-shot Node init every build-time core consumer uses: `loadCore()`
// fetches an absolute URL, which no Node process can resolve.
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { decidePopupRequest } from './core/dperm-popup';
import { popupCloseSettlement } from './core/dperm-connect';
import { toWireGrant } from './core/dperm-types';
import {
	CLOSED_WITHOUT_ANSWER,
	CONTENT_GRACE_MS,
	REQUEST_TTL_MS,
	SETTLE,
	instantConnectAnswer,
	resolveGrantedAccounts
} from '../../../extension/lib/protocol.js';
import { encode } from './request';
import { signRequestTtlMs } from '$lib/core/kernels';
import type { DAppGrant } from './grants';

const ALICE = `0x${'a1'.repeat(20)}`;
const BOB = `0x${'b2'.repeat(20)}`;

const grantFor = (address: string): DAppGrant => ({
	origin: 'https://app.example',
	address,
	chainId: 100,
	grantedAt: 1_700_000_000_000
});

describe('what a granted origin may see', () => {
	/** Every case that distinguishes the rule, including the load-bearing one. */
	const matrix: { name: string; grant: DAppGrant | null; addresses: string[] | null }[] = [
		{ name: 'no grant at all', grant: null, addresses: [ALICE] },
		{ name: 'grant, account present', grant: grantFor(ALICE), addresses: [ALICE, BOB] },
		{ name: 'grant, account gone', grant: grantFor(ALICE), addresses: [BOB] },
		// The one that matters most: a cold read must not be read as "the account
		// is gone", or every browser start logs the person out of every dApp.
		{ name: 'grant, addresses not known yet (null)', grant: grantFor(ALICE), addresses: null },
		{ name: 'grant, addresses not known yet (empty)', grant: grantFor(ALICE), addresses: [] },
		{ name: 'grant, different case', grant: grantFor(ALICE.toUpperCase()), addresses: [ALICE] }
	];

	for (const { name, grant, addresses } of matrix) {
		it(`agrees with the core: ${name}`, () => {
			const fromCore = decidePopupRequest({
				method: 'eth_accounts',
				grant: toWireGrant(grant),
				currentAddresses: addresses,
				pinnedAddress: null
			}).granted;
			const fromWorker = resolveGrantedAccounts(toWireGrant(grant), addresses);
			expect(fromWorker.map((a: string) => a.toLowerCase())).toEqual(
				fromCore.map((a) => a.toLowerCase())
			);
		});

		// Spec 089: a connect the worker answers without a surface is answered
		// exactly as the surface would — `Respond` with the core's payload — and
		// sent to a person exactly when the core says `Consent`.
		for (const method of ['eth_requestAccounts', 'wallet_requestPermissions']) {
			it(`answers ${method} as the core does: ${name}`, () => {
				const verdict = decidePopupRequest({
					method,
					grant: toWireGrant(grant),
					currentAddresses: addresses,
					pinnedAddress: null
				});
				const fromWorker = instantConnectAnswer(
					method,
					resolveGrantedAccounts(toWireGrant(grant), addresses)
				);
				const lower = (value: unknown) => JSON.stringify(value).toLowerCase();
				if (verdict.outcome.type === 'respond') {
					expect(lower(fromWorker)).toEqual(lower(encode(verdict.outcome.payload)));
				} else {
					expect(verdict.outcome.type).toBe('consent');
					expect(fromWorker).toBeNull();
				}
			});
		}
	}
});

describe('how a torn-down window settles', () => {
	it('is the core’s code, and it is NOT 4001', () => {
		const settlement = popupCloseSettlement();
		// 4001 would tell the dApp "nothing happened", and it would re-send —
		// double-spending an operation that may already be at the bundler.
		expect(settlement.code).not.toBe(4001);
		expect(CLOSED_WITHOUT_ANSWER.code).toBe(settlement.code);
	});
});

describe('the request lifecycle, pinned to the core (spec 082 RB6, RB11, contract §13)', () => {
	it('the worker’s 5-minute limit is the core’s `EXTENSION_REQUEST_TTL_MS`', () => {
		expect(REQUEST_TTL_MS).toBe(signRequestTtlMs());
	});

	it('the page gives up strictly AFTER the worker does', () => {
		expect(CONTENT_GRACE_MS).toBeGreaterThan(0);
		expect(REQUEST_TTL_MS + CONTENT_GRACE_MS).toBeGreaterThan(signRequestTtlMs());
	});

	it('every way a request ends without a decision is the core’s code, never 4001', () => {
		const code = popupCloseSettlement().code;
		expect(Object.keys(SETTLE).sort()).toEqual(
			['expired', 'page_left', 'restarted', 'surface_closed', 'updated'].sort()
		);
		for (const [cause, settle] of Object.entries(SETTLE)) {
			expect(settle.code, cause).toBe(code);
			expect(settle.code, cause).not.toBe(4001);
			expect(settle.message.length, cause).toBeGreaterThan(0);
		}
	});
});
