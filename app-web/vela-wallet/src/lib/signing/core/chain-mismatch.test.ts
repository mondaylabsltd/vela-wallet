/**
 * 089 — a request whose `chainId` differs from the site's chain is refused,
 * never sent on the other chain.
 *
 * The extension's request window and the web popup stamp the SITE's chain on
 * every request (`DappRequestHost` → `per_request_chain`). A page that wrote
 * `chainId: 0x64` while the site is on Ethereum in the wallet used to get an
 * Ethereum sheet — and, on this shell, a submit built for Gnosis
 * (`dapp-submit`'s `resolveChainId` lets the embedded chain win). The core now
 * answers -32602 before any sheet. Driven through the REAL core (wasm) and the
 * real executor, so the words the page reads are pinned to the core's
 * `CHAIN_MISMATCH_MESSAGE`.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';

const submit = vi.hoisted(() => ({ calls: 0 }));
vi.mock('$lib/services/dapp-submit', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/services/dapp-submit')>();
	return {
		...actual,
		handleDAppRequest: async () => {
			submit.calls += 1;
			return '0x' + '00'.repeat(32);
		}
	};
});

import type { SignView } from '$lib/core/generated/SignView';
import { createSignRequestSession } from './sign-session';
import { CHAIN_MISMATCH_MESSAGE, signErrorMessage, type SignShellPorts } from './sign-types';

const ACCOUNT = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';
const TO = '0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141';

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

/** One request, as `DappRequestHost` hands it to the machine. */
async function arrive(method: string, params: unknown[], siteChain: number | null) {
	const answers: Answer[] = [];
	let view: SignView | null = null;
	const faults: unknown[] = [];
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
		id: 'rid-89',
		method,
		params_json: JSON.stringify(params),
		origin: 'https://app.example',
		transport_id: 'ext-89',
		dedicated_transport: true,
		per_request_chain: siteChain,
		dapp: null,
		granted_address: ACCOUNT,
		requested_address: null,
		request_ts_ms: null,
		now_ms: Date.now()
	});
	await new Promise((resolve) => setTimeout(resolve, 0));
	session.dispose();
	expect(faults).toEqual([]);
	return { answers, view: view as SignView | null };
}

const tx = (chainId?: unknown) => [
	{ from: ACCOUNT, to: TO, value: '0x1', ...(chainId === undefined ? {} : { chainId }) }
];
const batch = (chainId: unknown) => [
	{ version: '2.0.0', from: ACCOUNT, chainId, calls: [{ to: TO, value: '0x1' }] }
];
const typed = (chainId: unknown) => [
	ACCOUNT,
	JSON.stringify({
		types: {
			EIP712Domain: [{ name: 'chainId', type: 'uint256' }],
			Mail: [{ name: 'contents', type: 'string' }]
		},
		primaryType: 'Mail',
		domain: { chainId },
		message: { contents: 'hi' }
	})
];

describe('a request naming another chain than the site’s (089)', () => {
	for (const [method, params] of [
		['eth_sendTransaction', tx('0x64')],
		['wallet_sendCalls', batch('0x64')],
		['eth_signTypedData_v4', typed(100)]
	] as const) {
		it(`${method}: site on Ethereum, request on Gnosis → -32602 before any sheet`, async () => {
			submit.calls = 0;
			const { answers, view } = await arrive(method, [...params], 1);
			expect(answers).toEqual([
				{
					id: 'rid-89',
					result: undefined,
					error: {
						code: -32602,
						kind: 'invalid_params',
						message: `Invalid params: ${CHAIN_MISMATCH_MESSAGE}`
					}
				}
			]);
			expect(view?.surface).toBe('hidden');
			expect(view?.request).toBeNull();
			expect(submit.calls).toBe(0);
		});
	}

	it('the words are the core’s own constant, carried as its detail', async () => {
		// The core sends `CHAIN_MISMATCH_MESSAGE` as the refusal's detail; this
		// side recognises exactly that string. A drift in either would fall back
		// to a bare "Invalid params" — caught here.
		expect(signErrorMessage({ kind: 'invalid_params', detail: CHAIN_MISMATCH_MESSAGE })).toBe(
			`Invalid params: ${CHAIN_MISMATCH_MESSAGE}`
		);
		const { answers } = await arrive('eth_sendTransaction', tx('0x64'), 1);
		expect(answers[0]?.error?.message).toContain(CHAIN_MISMATCH_MESSAGE);
	});

	for (const [name, method, params] of [
		['a matching chainId', 'eth_sendTransaction', tx('0x64')],
		['a matching decimal chainId', 'eth_sendTransaction', tx(100)],
		['no chainId at all', 'eth_sendTransaction', tx()],
		['a matching batch', 'wallet_sendCalls', batch('0x64')],
		['a matching typed-data domain', 'eth_signTypedData_v4', typed('0x64')]
	] as const) {
		it(`${name} reaches the sheet on the site’s chain`, async () => {
			const { answers, view } = await arrive(method, [...params], 100);
			expect(answers).toEqual([]);
			expect(view?.surface).toBe('sheet');
			expect(view?.request?.chain_id).toBe(100);
		});
	}

	it('with no stamp (the in-page path) the request still moves the chain, unchanged', async () => {
		const { answers, view } = await arrive('eth_sendTransaction', tx('0x64'), null);
		expect(answers).toEqual([]);
		expect(view?.surface).toBe('sheet');
		expect(view?.request?.chain_id).toBe(100);
		expect(view?.global_chain_id).toBe(100);
	});
});
