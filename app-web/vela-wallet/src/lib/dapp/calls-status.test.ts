/**
 * The worker's EIP-5792 answers, pinned to the core's (spec 094 S6).
 *
 * `wallet_getCallsStatus` and `wallet_getCapabilities` are answered by the
 * core's `dapp_rpc` rules in every in-app browser; the extension's service
 * worker cannot run the core, so it keeps twins in `extension/lib/protocol.js`.
 * These drive the REAL core over the same inputs and demand the same answers —
 * a twin that drifts is a dApp told one thing in Chrome and another on a phone.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import {
	dappRpcCallsStatus,
	dappRpcCallsStatusId,
	dappRpcCapabilities
} from '../../../../../rust/pkg-web/vela_core.js';
import {
	UNKNOWN_BUNDLE_ID,
	callsStatusId,
	callsStatusResult,
	capabilitiesResult,
	catalogChainIds
} from '../../../extension/lib/protocol.js';

const ID = `0x${'ab'.repeat(32)}`;
const ALICE = `0x${'a1'.repeat(20)}`;
const BOB = `0x${'b2'.repeat(20)}`;
const LOG = { address: '0x01', topics: [], data: '0x' };
const BUNDLE_LOG = { address: '0x02', topics: [], data: '0x' };
const TX = { transactionHash: `0x${'cd'.repeat(32)}`, blockHash: '0xbb', blockNumber: '0x10' };

describe('the batch id', () => {
	for (const params of [
		[ID],
		[ID.toUpperCase().replace('0X', '0x')],
		[],
		['0x12'],
		[{ id: ID }],
		[`${ID}00`],
		'not a list',
		null
	]) {
		it(`agrees on ${JSON.stringify(params)}`, () => {
			expect(callsStatusId(params)).toBe(dappRpcCallsStatusId(JSON.stringify(params)) ?? null);
		});
	}
});

describe('a batch’s status', () => {
	const receipts: [string, unknown][] = [
		['none yet', null],
		['an empty result', {}],
		['no transaction yet', { receipt: {} }],
		[
			'landed',
			{ success: true, logs: [LOG], receipt: { ...TX, status: '0x1', logs: [LOG, BUNDLE_LOG] } }
		],
		['landed, op logs only on the receipt', { success: true, receipt: { ...TX, logs: [LOG] } }],
		['reverted in a good bundle', { success: false, receipt: { ...TX, status: '0x1', logs: [] } }],
		['success unsaid', { receipt: { ...TX } }],
		['success as text', { success: 'false', receipt: { ...TX } }],
		['a list', [TX]]
	];
	for (const [name, receipt] of receipts) {
		it(`agrees: ${name}`, () => {
			const core = JSON.parse(
				dappRpcCallsStatus(ID, 100, receipt === null ? undefined : JSON.stringify(receipt))
			);
			expect(callsStatusResult(ID, 100, receipt)).toEqual(core);
		});
	}

	it('names the chain the batch went to', () => {
		expect(callsStatusResult(ID, 8453, null)).toMatchObject({ chainId: '0x2105', status: 100 });
	});
});

describe('the capabilities', () => {
	const cases: [string, unknown, string[]][] = [
		['the connected account', [ALICE], [ALICE]],
		['another case', [ALICE.toUpperCase().replace('0X', '0x')], [ALICE]],
		['named chains', [ALICE, ['0x64', '0x2105', '100', 1, 'zz', 1.5, -1]], [ALICE]],
		['an empty chain list', [ALICE, []], [ALICE]],
		['another account', [BOB], [ALICE]],
		['nobody connected', [ALICE], []],
		['no address', [], [ALICE]],
		['not an address', ['0x12'], [ALICE]]
	];
	const chains = [1, 100, 8453];
	for (const [name, params, granted] of cases) {
		it(`agrees: ${name}`, () => {
			const core = JSON.parse(
				dappRpcCapabilities(JSON.stringify(params), JSON.stringify(granted), JSON.stringify(chains))
			);
			const twin = capabilitiesResult(params, granted, chains);
			if (core.error) expect(twin.error?.code).toBe(core.error.code);
			else expect(twin).toEqual(core);
		});
	}
});

describe('the rest of the worker’s half', () => {
	it('reads the chain ids out of the catalog the wallet published', () => {
		expect(catalogChainIds({ chains: { '1': {}, '100': {}, x: {} } })).toEqual([1, 100]);
		expect(catalogChainIds(null)).toEqual([]);
	});

	it('uses EIP-5792’s code for a batch it never sent', () => {
		expect(UNKNOWN_BUNDLE_ID).toBe(5730);
	});
});
