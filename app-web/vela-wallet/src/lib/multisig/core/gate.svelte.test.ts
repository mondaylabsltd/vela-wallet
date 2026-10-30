/**
 * §3.5, in a browser (where `$state` proxies what it holds): an answered
 * confirmation clears itself, so nothing stays on screen after "Don't sign"
 * or "sign" — and the next signature asks a new one.
 */
import { describe, expect, it, vi } from 'vitest';

const { riskConfirmation } = await import('./gate.svelte');

const finding = {
	from: 'chain',
	chainId: 100,
	risk: { kind: 'module', address: '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1' }
} as const;

describe('the risk confirmation (§3.5)', () => {
	it('is gone once answered, either way, and asked anew next time', async () => {
		const detach = riskConfirmation.attach();
		const ask = () => riskConfirmation.ask({ chainId: 100, safe: '0x1', findings: [finding] });
		const declined = ask();
		riskConfirmation.request!.answer(false);
		await expect(declined).resolves.toBe(false);
		expect(riskConfirmation.request).toBeNull();
		const signed = ask();
		expect(riskConfirmation.request).not.toBeNull();
		riskConfirmation.request!.answer(true);
		await expect(signed).resolves.toBe(true);
		expect(riskConfirmation.request).toBeNull();
		detach();
	});
});
