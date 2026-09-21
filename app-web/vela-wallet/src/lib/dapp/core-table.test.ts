/**
 * The service worker's routing table, pinned to the core's (spec 070).
 *
 * The in-app browsers on desktop, iOS and Android route every provider request
 * through `vela_core::app::dapp_rpc::classify`; the extension's service worker
 * cannot run the core on every page load, so it keeps `classifyMethod` in
 * `extension/lib/protocol.js`. Two tables that LOOK like one are the bug this
 * file exists to catch: every method name either side knows goes through both,
 * and the buckets must agree.
 *
 * The page-side provider has one home too — `rust/crates/vela-core/provider/
 * inpage.js`, bundled by the extension and embedded by the core — and its six
 * constants must equal the ones the worker reads.
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { dappProviderScript, dappRpcClassify } from '../../../../../rust/pkg-web/vela_core.js';
import {
	BUNDLER_METHODS,
	CHANNEL,
	ERR,
	RDNS,
	READ_PROXY_METHODS,
	WALLET_NAME,
	classifyMethod
} from '../../../extension/lib/protocol.js';

/** The core's route, in the worker's bucket vocabulary. */
function bucketOf(method: string): string {
	const route = JSON.parse(dappRpcClassify(method)) as { type: string; bundler?: boolean };
	switch (route.type) {
		case 'connect':
			return 'connect';
		case 'accounts':
		case 'coinbase':
		case 'permissions':
		case 'chain_id':
		case 'net_version':
			return 'state';
		case 'revoke_permissions':
			return 'revoke';
		case 'switch_chain':
			return 'switch';
		case 'add_chain':
			return 'addChain';
		case 'watch_asset':
			return 'watchAsset';
		case 'sign':
			return 'sign';
		case 'read':
			return 'read';
		default:
			return 'unsupported';
	}
}

const EVERY_METHOD = [
	...READ_PROXY_METHODS,
	'eth_requestAccounts',
	'wallet_requestPermissions',
	'eth_accounts',
	'eth_coinbase',
	'wallet_getPermissions',
	'wallet_revokePermissions',
	'eth_chainId',
	'net_version',
	'wallet_switchEthereumChain',
	'wallet_addEthereumChain',
	'wallet_watchAsset',
	'eth_sendTransaction',
	'wallet_sendCalls',
	'personal_sign',
	'eth_signTypedData',
	'eth_signTypedData_v1',
	'eth_signTypedData_v3',
	'eth_signTypedData_v4',
	'eth_sign',
	'eth_signTransaction',
	'personal_ecRecover',
	'debug_traceCall',
	'wallet_getCallsStatus',
	''
];

describe('the worker routes exactly as the core does', () => {
	for (const method of EVERY_METHOD) {
		it(`agrees on ${method || '(empty)'}`, () => {
			expect(classifyMethod(method)).toBe(bucketOf(method));
		});
	}

	it('sends the same methods to the bundler', () => {
		for (const method of READ_PROXY_METHODS) {
			const route = JSON.parse(dappRpcClassify(method)) as { type: string; bundler?: boolean };
			expect(route.bundler, method).toBe(BUNDLER_METHODS.has(method));
		}
	});
});

describe('the provider has one home', () => {
	const provider = readFileSync(
		join(process.cwd(), '..', '..', 'rust', 'crates', 'vela-core', 'provider', 'inpage.js'),
		'utf8'
	);

	it('declares the constants the worker reads', () => {
		expect(provider).toContain(`const CHANNEL = '${CHANNEL}';`);
		expect(provider).toContain(`const RDNS = '${RDNS}';`);
		expect(provider).toContain(`const WALLET_NAME = '${WALLET_NAME}';`);
		for (const [name, code] of Object.entries(ERR)) expect(provider).toContain(`${name}: ${code}`);
	});

	it('is what every in-app browser injects', () => {
		for (const host of ['android', 'ios', 'desktop']) {
			const script = dappProviderScript(host);
			expect(script).toContain(provider.trim().slice(-200));
			expect(script.startsWith('(function () {\n\tif (window.top !== window) return;')).toBe(true);
		}
	});
});
