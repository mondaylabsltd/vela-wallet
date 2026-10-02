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
import { runInNewContext } from 'node:vm';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
	dappOffersWallet,
	dappProviderScript,
	dappRpcClassify
} from '../../../../../rust/pkg-web/vela_core.js';
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
			const script = dappProviderScript(host, false);
			expect(script).toContain(provider.trim().slice(-200));
			// Top frame only, and only a secure context (https, or http on
			// loopback) gets a provider at all (spec 088 FR-004).
			expect(
				script.startsWith(
					'(function () {\n\tif (window.top !== window || !window.isSecureContext) return;'
				)
			).toBe(true);
			// Debug mode (spec 091) changes the gate and nothing else.
			const debug = dappProviderScript(host, true);
			expect(debug).toContain(provider.trim().slice(-200));
			expect(debug.split(' return;\n').slice(1).join(' return;\n')).toBe(
				script.split(' return;\n').slice(1).join(' return;\n')
			);
		}
	});
});

/**
 * Spec 082 RG10 (L-D6): the page hears an account in the spelling the wallet
 * sent — EIP-55 — and the same account in another case is not a change.
 */
describe('the provider keeps one spelling of an account', () => {
	const source = readFileSync(
		join(process.cwd(), '..', '..', 'rust', 'crates', 'vela-core', 'provider', 'inpage.js'),
		'utf8'
	);
	const SPELLED = '0x7687c0bc1DD2b9d7E9A5b1B4e1b0CBd8E0C3D141';

	function page() {
		const listeners = new Map<string, ((ev: { source: unknown; data: unknown }) => void)[]>();
		const win: Record<string, unknown> = {
			location: { origin: 'https://a.example' },
			addEventListener: (type: string, fn: (ev: { source: unknown; data: unknown }) => void) =>
				listeners.set(type, [...(listeners.get(type) ?? []), fn]),
			dispatchEvent: () => true,
			postMessage: () => {}
		};
		const context = {
			window: win,
			document: { documentElement: { setAttribute: () => {} } },
			Event: class {},
			CustomEvent: class {},
			console: { log: () => {} },
			Promise,
			Map,
			Set,
			Object,
			Array,
			Error
		};
		runInNewContext(source, context);
		const push = (event: string, data: unknown) =>
			(listeners.get('message') ?? []).forEach((fn) =>
				fn({ source: win, data: { ch: CHANNEL, dir: 'evt', event, data } })
			);
		const provider = win.ethereum as {
			on(event: string, fn: (value: unknown) => void): void;
			selectedAddress: string | null;
		};
		return { provider, push };
	}

	it('accountsChanged carries the EIP-55 spelling it was sent', () => {
		const { provider, push } = page();
		const heard: unknown[] = [];
		provider.on('accountsChanged', (accounts) => heard.push(accounts));
		push('accountsChanged', [SPELLED]);
		expect(heard).toEqual([[SPELLED]]);
		expect(provider.selectedAddress).toBe(SPELLED);
	});

	it('the same account in another case fires no event', () => {
		const { provider, push } = page();
		const heard: unknown[] = [];
		provider.on('accountsChanged', (accounts) => heard.push(accounts));
		push('accountsChanged', [SPELLED]);
		push('accountsChanged', [SPELLED.toLowerCase()]);
		expect(heard).toHaveLength(1);
	});
});

/**
 * Spec 091: with debug mode on, the injected script tests the page's host in
 * JavaScript — a test the CORE writes out of its own tables. Here it runs, as
 * a WebView runs it, against the core's rule (`dappOffersWallet`) on a table
 * of hosts: the script installs the bridge (its hello reaches the host)
 * exactly where the machine's gate lets the page in.
 *
 * What the page sees is what the platform reports: `location` and the frame
 * origin are both the URL parser's normalised spelling, so each origin goes
 * through `new URL` first, and one the parser refuses is no page at all.
 * `window.isSecureContext` is the platform's own answer; it is stood in for by
 * the core's spec-088 rule, which is what spec 088 holds it to.
 */
describe('debug mode: the script and the gate offer the wallet to the same pages', () => {
	const ORIGINS = [
		// Secure contexts.
		'https://dapp.example',
		'https://192.168.1.5:3000',
		'http://localhost:5173',
		'http://dev.localhost',
		'http://127.0.0.1:8137',
		'http://127.12.0.9',
		'http://[::1]:3000',
		// This device's network: offered in debug mode only.
		'http://192.168.1.5:3000',
		'http://192.168.0.1',
		'http://10.0.0.1',
		'http://10.255.255.255',
		'http://172.16.0.1',
		'http://172.31.255.255',
		'http://169.254.1.1',
		'http://foo.local',
		'http://FOO.LOCAL:8080',
		'http://printer.office.local',
		'http://[fd00::1]',
		'http://[fd00::1]:3000',
		'http://[fc12:3456::1]',
		'http://[fdff:ffff::1]',
		'http://[fe80::2]',
		'http://[FE80::1]',
		// Shorthand the URL parser normalises before anyone sees it.
		'http://192.168.1',
		'http://0x7f.1',
		'http://10.1',
		// Public: never.
		'http://dapp.example',
		'http://10.0.0.1.evil.com',
		'http://192.168.1.5.nip.io',
		'http://127.0.0.1.evil.com',
		'http://localhost.evil.com',
		'http://foo.local.evil.com',
		'http://local',
		'http://localhostx',
		'http://172.15.0.1',
		'http://172.32.0.1',
		'http://192.169.0.1',
		'http://169.253.0.1',
		'http://11.0.0.1',
		'http://8.8.8.8',
		'http://1.1.1.1',
		'http://[2001:db8::1]',
		'http://[fe81::1]',
		'http://[fec0::1]',
		'http://[fb00::1]',
		'http://[fe00::1]',
		'http://[::ffff:192.168.1.5]',
		'http://[::]',
		'http://0.0.0.0',
		// Refused by the parser: not a page.
		'http://999.1.1.1',
		'http://[fd00::1'
	];

	/** Run the injected script in a page at `url`; `true` when it said hello. */
	function installs(script: string, url: URL, secure: boolean): boolean {
		const posts: { t?: string }[] = [];
		const location = { href: url.href, protocol: url.protocol, hostname: url.hostname, origin: url.origin };
		const win: Record<string, unknown> = {
			location,
			isSecureContext: secure,
			crypto: { randomUUID: () => 'doc-1' },
			ipc: { postMessage: (s: string) => posts.push(JSON.parse(s)) },
			addEventListener: () => {},
			dispatchEvent: () => true,
			postMessage: () => {}
		};
		win.top = win;
		runInNewContext(script, {
			window: win,
			location,
			document: { documentElement: { setAttribute: () => {} } },
			Event: class {},
			CustomEvent: class {},
			console: { log: () => {}, error: () => {} }
		});
		return posts.some((post) => post.t === 'hello');
	}

	for (const debugMode of [false, true]) {
		it(`debug mode ${debugMode ? 'on' : 'off'}`, () => {
			const script = dappProviderScript('desktop', debugMode);
			let pages = 0;
			for (const origin of ORIGINS) {
				let url: URL;
				try {
					url = new URL(origin);
				} catch {
					continue;
				}
				pages += 1;
				const secure = dappOffersWallet(url.origin, false);
				expect(installs(script, url, secure), `${origin} (${url.origin})`).toBe(
					dappOffersWallet(url.origin, debugMode)
				);
			}
			expect(pages).toBe(ORIGINS.length - 2);
		});
	}

	it('offers what the ruling says, and only that', () => {
		const offered = (origin: string) => dappOffersWallet(new URL(origin).origin, true);
		for (const lan of ['http://192.168.1.5:3000', 'http://[fd00::1]', 'http://foo.local']) {
			expect(offered(lan), lan).toBe(true);
			expect(dappOffersWallet(new URL(lan).origin, false), lan).toBe(false);
		}
		for (const pub of ['http://10.0.0.1.evil.com', 'http://dapp.example', 'http://8.8.8.8']) {
			expect(offered(pub), pub).toBe(false);
		}
	});
});
