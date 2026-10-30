/**
 * §3.5, §3.6: whether this device signs for a multisig on a network — the
 * core's rule (`multisig_review::signing_gate`), as the sheets read it.
 * Read-only where its genesis is refused; no signature at all for a
 * transaction whose execution changes who controls it undeclared; where more
 * than its members can act for it, a risk. The gate before each signature —
 * the chain read again, the risk asked every time, nothing signed unchecked
 * — is the proposals machine's (Rust: `a_refused_genesis_signs_nothing`,
 * `a_risk_is_asked_every_signature`, `an_unchecked_network_signs_nothing_yet`).
 */
import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../../rust/pkg-web/vela_core_wasm_url.js';

const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';

const { gateOf } = await import('./gate.svelte');

const risk = { from: 'chain', chainId: 100, risk: { kind: 'module', address: MODULE } } as const;
const refusal = {
	from: 'genesis',
	chainId: 100,
	reason: { kind: 'module', address: MODULE }
} as const;

beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

describe('the gate a network makes — the core rule (§3.5, §3.6)', () => {
	it('is read per network: a refusal comes before a risk; another network says nothing', () => {
		expect(gateOf([risk], 100)).toEqual({ kind: 'risk', chainId: 100, findings: [risk] });
		expect(gateOf([risk, refusal], 100)).toEqual({
			kind: 'refused',
			chainId: 100,
			findings: [refusal]
		});
		expect(gateOf([risk, refusal], 8453)).toEqual({ kind: 'open' });
	});

	it('a transaction Vela does not sign is blocked, with or without a risk; "anyway" stays open', () => {
		const blocked = { allowed: false, anyway: false };
		expect(gateOf([], 100, blocked)).toEqual({ kind: 'blocked', chainId: 100 });
		expect(gateOf([risk], 100, blocked)).toEqual({ kind: 'blocked', chainId: 100 });
		// A refused network still says so first: read-only there.
		expect(gateOf([refusal], 100, blocked).kind).toBe('refused');
		expect(gateOf([], 100, { allowed: true, anyway: true })).toEqual({ kind: 'open' });
	});
});
