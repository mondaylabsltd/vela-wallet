/**
 * Spec 102 (P2b-W1) — an account that cannot sign on the web, end to end.
 *
 * The web opens no signing page, so an account whose keys live on a custom
 * signing domain cannot sign here (`signingPlan(record, 'web')` comes back
 * `blocked: not_on_web`). The refusal is thrown before any ceremony and the
 * executor hands it to the core as `venue_blocked { block }`. Driven through
 * the REAL core (wasm) and the real executor, this pins what the person and
 * the page each get:
 *
 * - the sheet's failed status says the core's reason in the person's
 *   language (`SignErrorNotice.venue_block` → `settings.venue.blockedWeb`),
 *   never an English sentence of ours, and offers no "Try again" — the same
 *   account would be refused the same way;
 * - the page, once the person closes the sheet, hears -32603 with the core's
 *   calm English ("This account cannot sign here").
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/services/dapp-submit', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/services/dapp-submit')>();
	const { VenueBlockedError } = await import('$lib/signing/sign-challenge');
	return {
		...actual,
		handleDAppRequest: async () => {
			throw new VenueBlockedError({ type: 'not_on_web' });
		}
	};
});

import type { SignView } from '$lib/core/generated/SignView';
import { resolveSigningMessages } from '$lib/i18n/engine.server';
import { signingStatus } from '../live';
import { createSignRequestSession } from './sign-session';
import type { SignShellPorts } from './sign-types';

const ACCOUNT = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';

type Answer = {
	id: string;
	result?: unknown;
	error?: { code: number; message: string; kind?: string };
};

function ports(answers: Answer[]): SignShellPorts {
	return {
		transportFor: () => ({
			sendResponse: (id, result, error) => answers.push({ id, result, error })
		}),
		opSubmitted: () => {},
		opSigned: () => {},
		ceremony: () => {},
		askerLive: async () => true,
		approvedAtMs: () => null,
		requestOrigin: () => 'https://app.example',
		assetSim: () => null,
		switchActiveAccount: async () => {},
		recordsWritten: () => {}
	};
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** A message to sign arrives, the person confirms it, and the web refuses the venue. */
async function signAndRefuse() {
	const answers: Answer[] = [];
	const faults: unknown[] = [];
	let view: SignView | null = null;
	const session = createSignRequestSession({
		ports: ports(answers),
		onView: (next) => (view = next),
		onError: (error) => faults.push(error)
	});
	session.start({ type: 'networks_changed', chain_ids: [1, 100] });
	session.dispatch({
		type: 'accounts_changed',
		accounts: [{ address: ACCOUNT, credential_id: 'cred-1' }],
		active_index: 0
	});
	session.dispatch({
		type: 'request_arrived',
		id: 'rid-102',
		method: 'personal_sign',
		params_json: JSON.stringify(['0x68656c6c6f', ACCOUNT]),
		origin: 'https://app.example',
		transport_id: 'ext-102',
		dedicated_transport: true,
		per_request_chain: 100,
		dapp: null,
		granted_address: ACCOUNT,
		requested_address: null,
		request_ts_ms: null,
		now_ms: Date.now(),
		first_party: false
	});
	await settle();
	session.dispatch({
		type: 'approve_tapped',
		opts: {
			max_fee_per_gas: null,
			bundler_cost_wei: null,
			gas_fee_token: null,
			quoted_fee: null,
			fee_collector: null,
			params_override_json: null,
			intent: null,
			unlimited_approved: false
		}
	});
	await vi.waitFor(() => expect((view as SignView | null)?.error).not.toBeNull());
	return {
		answers,
		faults,
		view: () => view as SignView,
		close: async () => {
			session.dispatch({ type: 'dismiss_tapped' });
			await settle();
			session.dispose();
		}
	};
}

describe('an account that cannot sign on the web (P2b-W1)', () => {
	it('the sheet says the core’s reason, in the person’s language, with no Try again', async () => {
		const run = await signAndRefuse();
		const view = run.view();
		expect(view.error).toMatchObject({
			kind: 'venue_blocked',
			venue_block: { type: 'not_on_web' }
		});
		expect(view.failure_retryable).toBe(false);
		for (const locale of ['zh', 'en'] as const) {
			const m = resolveSigningMessages(locale);
			const status = signingStatus(view, undefined, undefined, m);
			expect(status, locale).toMatchObject({
				stage: 'failed',
				title: m.receipt.failed,
				captions: [m.venueBlock.blockedWeb],
				actions: { close: m.receipt.done }
			});
			expect(status?.actions?.retry, locale).toBeUndefined();
		}
		expect(resolveSigningMessages('zh').venueBlock.blockedWeb).toBe(
			'签名页只能从 Vela 应用打开，网页版不支持。'
		);
		await run.close();
		expect(run.faults).toEqual([]);
	});

	it('the page hears -32603 once the sheet is closed — in the core’s words, not ours', async () => {
		const run = await signAndRefuse();
		// Held while the failure is on screen (spec 096 F8).
		expect(run.answers).toEqual([]);
		await run.close();
		expect(run.answers).toEqual([
			{
				id: 'rid-102',
				result: undefined,
				error: expect.objectContaining({
					code: -32603,
					message: 'This account cannot sign here'
				})
			}
		]);
	});
});
