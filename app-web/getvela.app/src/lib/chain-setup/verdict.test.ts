import { describe, expect, it } from 'vitest';
import { REQUIRED_CONTRACTS } from './required-contracts';
import type { ContractStatus } from './rpc';
import { estimatedFundingWei, plan } from './verdict';

const status = (key: string, over: Partial<ContractStatus> = {}): ContractStatus => ({
	contract: REQUIRED_CONTRACTS.find((c) => c.key === key)!,
	deployed: true,
	mismatch: false,
	unknown: false,
	...over
});
const all = (missing: string[] = [], unknown: string[] = []) =>
	REQUIRED_CONTRACTS.map((c) =>
		status(c.key, {
			deployed: !missing.includes(c.key) && !unknown.includes(c.key),
			unknown: unknown.includes(c.key)
		})
	);

describe('the verdict', () => {
	it('is ready only when everything is present and P-256 answers', () => {
		expect(plan(all(), 'precompile').verdict).toBe('ready');
		expect(plan(all(), 'contract').verdict).toBe('ready');
	});

	it('is blocked by a missing P-256 no matter how complete the contracts are — and offers no steps', () => {
		const p = plan(all(), 'missing');
		expect(p.verdict).toBe('blocked');
		expect(p.steps).toEqual([]);
		expect(p.runnable).toEqual([]);
	});

	it('needs setup when something is missing, and lists only that', () => {
		const p = plan(all(['multiSend', 'entryPoint']), 'precompile');
		expect(p.verdict).toBe('needs-setup');
		expect(p.steps.map((s) => s.contract.key)).toEqual(['entryPoint', 'multiSend']);
	});

	it('a chain that is only missing the passkey signer is one step from whole', () => {
		// The gap this page used to have: it called such a chain ready, and a
		// person with two keys deposited into an address that could never be
		// deployed there. Now both contracts show as missing — and the plan
		// still holds ONE step, because the factory's constructor deploys the
		// singleton.
		const p = plan(all(['safePasskeySignerFactory', 'safePasskeySignerSingleton']), 'precompile');
		expect(p.verdict).toBe('needs-setup');
		expect(p.missing.map((c) => c.contract.key)).toEqual([
			'safePasskeySignerFactory',
			'safePasskeySignerSingleton'
		]);
		expect(p.steps.map((s) => s.contract.key)).toEqual(['safePasskeySignerFactory']);
		expect(p.runnable).toHaveLength(1);
	});

	it('does not call an unanswered read "missing" — the verdict stays provisional', () => {
		const p = plan(all([], ['multiSend']), 'precompile');
		expect(p.verdict).toBe('needs-setup');
		expect(p.missing).toEqual([]);
		expect(p.unknown.map((c) => c.contract.key)).toEqual(['multiSend']);
		expect(p.steps).toEqual([]);
	});
});

describe('the plan says who can do each step', () => {
	it('keyless contracts are fund-and-broadcast with their one-time sender', () => {
		const p = plan(all(['arachnidProxy', 'multicall3']), 'precompile');
		expect(p.steps.map((s) => s.kind)).toEqual(['fund-and-broadcast', 'fund-and-broadcast']);
		const [a, m] = p.steps as Extract<(typeof p.steps)[number], { kind: 'fund-and-broadcast' }>[];
		expect(a.deployer).toBe('0x3fab184622dc19b6109349b94811493bf2a45362');
		expect(a.fundingWei).toBe(10_000_000_000_000_000n);
		expect(m.deployer).toBe('0x05f32b3cc3888453ff71b01135b34ff8e41263f2');
		expect(m.fundingWei).toBe(100_000_000_000_000_000n);
	});

	it('the Safe singleton factory is external, and is not in the runnable set', () => {
		const p = plan(all(['safeSingletonFactory']), 'precompile');
		expect(p.steps[0]).toMatchObject({ kind: 'external' });
		expect(p.runnable).toEqual([]);
	});

	it('a CREATE2 step names the factory it needs, and waits on it when that factory is missing too', () => {
		const p = plan(all(['safeSingletonFactory', 'safeSingleton', 'entryPoint']), 'precompile');
		const safe = p.steps.find((s) => s.contract.key === 'safeSingleton');
		expect(safe).toMatchObject({ kind: 'create2', factory: 'safeSingletonFactory' });
		expect((safe as { blockedBy: { key: string } }).blockedBy.key).toBe('safeSingletonFactory');
		const ep = p.steps.find((s) => s.contract.key === 'entryPoint');
		expect(ep).toMatchObject({ kind: 'create2', factory: 'arachnid', blockedBy: null });
	});

	it('keeps the wallet’s dependency order: factories before what they deploy', () => {
		const p = plan(all(REQUIRED_CONTRACTS.map((c) => c.key)), 'precompile');
		const keys = p.steps.map((s) => s.contract.key);
		expect(keys.indexOf('arachnidProxy')).toBeLessThan(keys.indexOf('entryPoint'));
		expect(keys.indexOf('safeSingletonFactory')).toBeLessThan(keys.indexOf('safeSingleton'));
	});
});

describe('the funding estimate', () => {
	it('is sized from creation code, not a flat ceiling, and excludes the keyless steps', () => {
		const p = plan(all(['multiSend', 'entryPoint', 'multicall3']), 'precompile');
		const wei = estimatedFundingWei(p, 1_000_000_000n);
		// multiSend ≈ 0.21M and entryPoint ≈ 3.87M gas at 1 gwei, +25% — and not Multicall3's 0.1 coin.
		expect(wei).toBeGreaterThan(4_000_000_000_000_000n);
		expect(wei).toBeLessThan(6_000_000_000_000_000n);
	});
	it('is zero when nothing is left for the throwaway key to do', () => {
		expect(estimatedFundingWei(plan(all(['multicall3']), 'precompile'), 10n ** 9n)).toBe(0n);
	});
});
