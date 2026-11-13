/**
 * §3.5: the confirmation a signature on a high-risk network asks — what was
 * found there, by its address, and the person's explicit answer. Closing
 * it, or "Don't sign", signs nothing.
 */
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { MULTISIG_SHAPE, type MultisigMessages } from '../messages';

vi.mock('$lib/services/networks', () => ({ chainName: (id: number) => `Chain ${id}` }));

const { default: RiskConfirmSheet } = await import('./RiskConfirmSheet.svelte');

const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';

function words(): MultisigMessages {
	const out: Record<string, Record<string, string>> = {};
	for (const [group, keys] of Object.entries(MULTISIG_SHAPE)) {
		out[group] = {};
		for (const key of Object.keys(keys)) out[group][key] = `${group}.${key}`;
	}
	out.authority.signRiskBody = 'On {{network}}, more than its members act.';
	out.authority.riskModule = 'On {{network}}, a module: {{address}}';
	return out as unknown as MultisigMessages;
}

function show(answer: (signs: boolean) => void) {
	return render(RiskConfirmSheet, {
		props: {
			m: words(),
			request: {
				chainId: 100,
				safe: '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83',
				findings: [{ from: 'chain', chainId: 100, risk: { kind: 'module', address: MODULE } }],
				answer
			},
			wide: true,
			closeLabel: 'Close'
		}
	});
}

describe('signing for a high-risk multisig (§3.5)', () => {
	it('names the network and what was found there, by its address', async () => {
		const screen = show(() => {});
		const sheet = screen.getByTestId('risk-confirm');
		await expect.element(sheet).toHaveTextContent('On Chain 100, more than its members act.');
		await expect.element(sheet).toHaveTextContent(`On Chain 100, a module: ${MODULE}`);
	});

	it('signs only on the explicit tap', async () => {
		const answer = vi.fn();
		const screen = show(answer);
		await screen.getByRole('button', { name: 'authority.signRiskConfirm' }).click();
		expect(answer).toHaveBeenCalledWith(true);
	});

	it("Don't sign answers no", async () => {
		const answer = vi.fn();
		const screen = show(answer);
		await screen.getByRole('button', { name: 'authority.signRiskCancel' }).click();
		expect(answer).toHaveBeenCalledWith(false);
	});
});
