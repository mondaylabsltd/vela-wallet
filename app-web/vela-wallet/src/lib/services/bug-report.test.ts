/**
 * The report, and everything that must never be in it (spec 081 FR-016).
 *
 * The property under test is not "these five fields are present" — a payload
 * with the right shape and somebody's address inside it is exactly the failure
 * this feature exists to prevent. It is:
 *
 *   the consent line is literal (the preview IS the payload),
 *   nothing on the wallet's shelf can reach the wire, and
 *   a refused send still leaves the person a working road.
 *
 * The exclusion test seeds the device with the four forbidden classes —
 * an address, a balance, an RPC URL carrying an API key, and raw `vela.*`
 * values — and asserts none of them appear in the serialized payload.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	AREA_OTHER,
	buildBugReport,
	environmentLines,
	fingerprintOf,
	prefilledIssueURL,
	redact,
	sendBugReport,
	type DeviceFacts,
	type EnvironmentLabels
} from './bug-report';

const LABELS: EnvironmentLabels = {
	version: 'App version',
	platform: 'Platform',
	language: 'Language',
	rpc: 'Unreachable RPC',
	failures: 'Recent failures',
	none: 'None'
};

/** The four things a report may never carry, spelled out once. */
const SECRETS = {
	address: '0x44EEC06897ff7ab8C7f16819511A64bA168A6D33',
	balance: '1234.56789',
	rpcWithKey: 'https://eth-mainnet.g.alchemy.com/v2/SUPER-SECRET-KEY',
	storedValue: '{"accounts":[{"address":"0x44EEC06897ff7ab8C7f16819511A64bA168A6D33"}]}'
} as const;

const FACTS: DeviceFacts = {
	version: '1.0.0',
	commit: 'abc1234',
	platform: 'Web · Mozilla/5.0',
	language: 'en',
	unreachable: ['Gnosis'],
	failures: ['rpc:final_failure ×2']
};

afterEach(() => {
	vi.unstubAllGlobals();
});

describe('what a report is allowed to know', () => {
	it('carries five fields and no sixth', () => {
		const payload = buildBugReport({
			what: 'Send froze',
			steps: '1. tap send',
			area: AREA_OTHER,
			labels: LABELS,
			facts: FACTS
		});
		expect(Object.keys(payload).sort()).toEqual([
			'area',
			'environment',
			'fingerprint',
			'steps',
			'what'
		]);
	});

	it('contains no address, balance, endpoint URL or stored value — even when the device is full of them', () => {
		// The whole shelf, present and readable, exactly as it is on a real
		// device. Nothing in the builder is given a way to reach it.
		vi.stubGlobal('localStorage', {
			length: 3,
			key: (i: number) => ['vela.accounts', 'vela.balanceCache', 'vela.networkConfig'][i] ?? null,
			getItem: () => SECRETS.storedValue,
			setItem: () => {},
			removeItem: () => {},
			clear: () => {}
		});

		const payload = buildBugReport({
			what: 'Balances stopped updating',
			steps: '1. open the wallet\n2. wait',
			area: AREA_OTHER,
			labels: LABELS,
			facts: FACTS
		});
		const wire = JSON.stringify(payload);

		expect(wire).not.toContain(SECRETS.address);
		expect(wire).not.toContain(SECRETS.balance);
		expect(wire).not.toContain(SECRETS.rpcWithKey);
		expect(wire).not.toContain('SUPER-SECRET-KEY');
		expect(wire).not.toContain(SECRETS.storedValue);
		// And no `vela.` key name either: a key list is a map of the shelf.
		expect(wire).not.toContain('vela.');
	});

	it('redacts an address or a URL that rode in on a name the person chose', () => {
		// A custom network can be named anything, including its own RPC URL —
		// which is how a key would otherwise reach a public issue tracker.
		const lines = environmentLines(LABELS, {
			...FACTS,
			unreachable: [SECRETS.rpcWithKey, SECRETS.address]
		});
		const joined = lines.join('\n');
		expect(joined).not.toContain('SUPER-SECRET-KEY');
		expect(joined).not.toContain(SECRETS.address);
		expect(joined).toContain('[url]');
		expect(joined).toContain('[address]');
	});

	it('redacts nothing it was not asked to', () => {
		expect(redact('Recent failures: rpc:final_failure ×2')).toBe(
			'Recent failures: rpc:final_failure ×2'
		);
	});

	it('says "none" rather than inventing a dash', () => {
		const lines = environmentLines(LABELS, { ...FACTS, unreachable: [], failures: [] });
		expect(lines[3]).toBe('Unreachable RPC: None');
		expect(lines[4]).toBe('Recent failures: None');
	});
});

describe('the preview is the payload (the consent line, made literal)', () => {
	it('sends exactly the lines the sheet showed', () => {
		const shown = environmentLines(LABELS, FACTS);
		const payload = buildBugReport({
			what: 'x',
			area: AREA_OTHER,
			labels: LABELS,
			facts: FACTS
		});
		expect(payload.environment).toBe(shown.join('\n'));
	});
});

describe('the fallback road', () => {
	it('prefills by the form’s FIELD IDS — `body` is ignored with template=bug.yml', () => {
		const payload = buildBugReport({
			what: 'Send froze',
			steps: '1. tap send',
			area: AREA_OTHER,
			labels: LABELS,
			facts: FACTS
		});
		const url = new URL(prefilledIssueURL(payload));
		expect(url.searchParams.get('template')).toBe('bug.yml');
		expect(url.searchParams.get('what')).toBe('Send froze');
		expect(url.searchParams.get('steps')).toBe('1. tap send');
		expect(url.searchParams.get('environment')).toBe(payload.environment);
		expect(url.searchParams.get('area')).toBe(AREA_OTHER);
		// The gotcha, asserted so it cannot come back: a `body` parameter here
		// would be silently dropped by GitHub and the form would open empty.
		expect(url.searchParams.get('body')).toBeNull();
	});

	it.each([
		[503, 'not_configured'],
		[429, 'rate_limited'],
		[413, 'too_large'],
		[500, 'rejected']
	])('falls back on %i rather than losing the report', async (status, reason) => {
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => new Response('{}', { status }))
		);
		const payload = buildBugReport({ what: 'x', area: AREA_OTHER, labels: LABELS, facts: FACTS });
		const outcome = await sendBugReport(payload, 'https://example.test/api/bug-report');
		expect(outcome.ok).toBe(false);
		if (!outcome.ok) {
			expect(outcome.reason).toBe(reason);
			expect(outcome.fallbackUrl).toContain('what=');
		}
	});

	it('falls back when the endpoint is unreachable', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => {
				throw new TypeError('network down');
			})
		);
		const payload = buildBugReport({ what: 'x', area: AREA_OTHER, labels: LABELS, facts: FACTS });
		const outcome = await sendBugReport(payload, 'https://example.test/api/bug-report');
		expect(outcome).toMatchObject({ ok: false, reason: 'unreachable' });
	});

	it('reports the issue it became when the endpoint files it', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(
				async () =>
					new Response(
						JSON.stringify({ number: 42, url: 'https://github.com/x/y/issues/42', deduped: true }),
						{ status: 200 }
					)
			)
		);
		const payload = buildBugReport({ what: 'x', area: AREA_OTHER, labels: LABELS, facts: FACTS });
		const outcome = await sendBugReport(payload, 'https://example.test/api/bug-report');
		expect(outcome).toEqual({
			ok: true,
			number: 42,
			url: 'https://github.com/x/y/issues/42',
			deduped: true
		});
	});
});

describe('the dedup marker', () => {
	it('is stable for the same complaint and different for another', () => {
		const a = fingerprintOf('Send froze', AREA_OTHER, '1.0.0');
		expect(fingerprintOf('  SEND FROZE ', AREA_OTHER, '1.0.0')).toBe(a);
		expect(fingerprintOf('Receive froze', AREA_OTHER, '1.0.0')).not.toBe(a);
	});
});
