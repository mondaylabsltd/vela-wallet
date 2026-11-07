/**
 * §28: a test build never writes to production — the web's guard, read
 * through the shipped wasm (the core's rule), with `fetch` answered here: the
 * production host names are a stand-in on this machine (`loopback`), so
 * nothing in this file can reach a production service.
 */
import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it } from 'vitest';
import * as wasm from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';
import { SERVICE_DEFAULTS } from '$lib/core/generated/serviceDefaults';
import { DEFAULT_SERVICE_ENDPOINTS } from './endpoints';
import { DEFAULT_REGISTRY_URL } from '$lib/onboarding/core/registry';
import { BUG_REPORT_ENDPOINT } from './bug-report';
import {
	ProductionWriteRefused,
	guardFetch,
	optedIn,
	type Decide,
	type GuardPolicy,
	type WriteVerdict
} from './production-guard';

const VECTOR = JSON.parse(
	readFileSync('../../rust/crates/vela-core/tests/vectors/rules/production-writes.json', 'utf8')
) as {
	defaults: Record<string, string>;
	optIn: [string | null, boolean][];
	cases: {
		about: string;
		testBuild: boolean;
		allowProductionWrites: boolean;
		method: string;
		url: string;
		body?: string;
		verdict: 'allowed' | 'refused';
		host?: string;
	}[];
};

beforeAll(() => {
	wasm.initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

/** The core's door, as the guard asks it. */
const core: Decide = async (ask) =>
	JSON.parse(wasm.productionWriteVerdict(JSON.stringify(ask))) as WriteVerdict;

/** The production names, answered on this machine: records what reached it. */
function loopback() {
	const seen: string[] = [];
	const inner = (async (input: RequestInfo | URL, init?: RequestInit) => {
		const url = new URL(input instanceof Request ? input.url : String(input));
		const method = init?.method ?? (input instanceof Request ? input.method : 'GET');
		seen.push(`${method} ${url.host}${url.pathname}`);
		return new Response('{"ok":true}', { status: 200 });
	}) as typeof fetch;
	return { seen, inner };
}

const TEST: GuardPolicy = { testBuild: true, allowProductionWrites: false };
const RELEASE: GuardPolicy = { testBuild: false, allowProductionWrites: false };
const REGISTER = 'https://p256-index-v2.getvela.app/api/register';

describe('a test build (§28)', () => {
	it('refuses a write to production before it leaves, honestly', async () => {
		const { seen, inner } = loopback();
		const guarded = guardFetch(inner, () => TEST, core);
		const refused = await guarded(REGISTER, { method: 'POST', body: '{"members":[]}' }).catch(
			(e: unknown) => e
		);
		expect(refused).toBeInstanceOf(ProductionWriteRefused);
		expect((refused as ProductionWriteRefused).host).toBe('p256-index-v2.getvela.app');
		expect((refused as Error).message).toContain(
			"This test build won't write to the live service; point it at a local one."
		);
		expect(seen).toEqual([]);
	});

	it('still reads production, and writes to a local stand-in', async () => {
		const { seen, inner } = loopback();
		const guarded = guardFetch(inner, () => TEST, core);
		await guarded('https://p256-index-v2.getvela.app/api/health');
		const read = '{"jsonrpc":"2.0","id":1,"method":"eth_estimateUserOperationGas","params":[]}';
		await guarded('https://vela-relay-cf.getvela.app/100', { method: 'POST', body: read });
		await guarded(
			new Request('http://127.0.0.1:9566/api/register', { method: 'POST', body: '{}' })
		);
		const send = '{"jsonrpc":"2.0","id":2,"method":"eth_sendUserOperation","params":[]}';
		await expect(
			guarded('https://vela-relay-cf.getvela.app/100', { method: 'POST', body: send })
		).rejects.toBeInstanceOf(ProductionWriteRefused);
		// A Request object's body is read too.
		await expect(
			guarded(
				new Request('https://safe-genesis.getvela.app/api/genesis', { method: 'POST', body: '{}' })
			)
		).rejects.toBeInstanceOf(ProductionWriteRefused);
		expect(seen).toEqual([
			'GET p256-index-v2.getvela.app/api/health',
			'POST vela-relay-cf.getvela.app/100',
			'POST 127.0.0.1:9566/api/register'
		]);
	});

	it('writes once opted in', async () => {
		const { seen, inner } = loopback();
		await guardFetch(
			inner,
			() => ({ testBuild: true, allowProductionWrites: true }),
			core
		)(REGISTER, { method: 'POST', body: '{}' });
		expect(seen).toEqual(['POST p256-index-v2.getvela.app/api/register']);
	});
});

describe('a release build', () => {
	it('is unaffected: its write leaves as before (to the loopback stand-in)', async () => {
		const { seen, inner } = loopback();
		let asked = 0;
		const counting: Decide = async (ask) => {
			asked += 1;
			return core(ask);
		};
		await guardFetch(
			inner,
			() => RELEASE,
			counting
		)(REGISTER, {
			method: 'POST',
			body: '{"members":[]}'
		});
		expect(seen).toEqual(['POST p256-index-v2.getvela.app/api/register']);
		expect(asked).toBe(0);
	});
});

describe('the core’s rule, through the wasm', () => {
	it('answers every vector case as the core says', async () => {
		for (const c of VECTOR.cases) {
			const verdict = await core({
				testBuild: c.testBuild,
				allowProductionWrites: c.allowProductionWrites,
				method: c.method,
				url: c.url,
				body: c.body
			});
			expect(verdict.verdict, c.about).toBe(c.verdict);
			if (verdict.verdict === 'refused') expect(verdict.host, c.about).toBe(c.host);
		}
	});

	it('reads the opt-in as the core does', () => {
		for (const [value, on] of VECTOR.optIn) expect(optedIn(value), String(value)).toBe(on);
	});

	it('the defaults are the core’s, with no copy of the web’s own', () => {
		expect(SERVICE_DEFAULTS).toEqual(VECTOR.defaults);
		expect(JSON.parse(wasm.serviceDefaults())).toEqual(VECTOR.defaults);
		expect(DEFAULT_SERVICE_ENDPOINTS.passkeyIndexURL).toBe(SERVICE_DEFAULTS.passkeyIndexUrl);
		expect(DEFAULT_SERVICE_ENDPOINTS.genesisServiceURL).toBe(SERVICE_DEFAULTS.genesisServiceUrl);
		expect(DEFAULT_REGISTRY_URL).toBe(SERVICE_DEFAULTS.passkeyIndexUrl);
		expect(BUG_REPORT_ENDPOINT).toBe(SERVICE_DEFAULTS.bugReportUrl);
	});
});
