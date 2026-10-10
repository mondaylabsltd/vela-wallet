/**
 * PR 3 final note F14, in a real browser: the add-token sheet's network tab
 * draws the RPC field and "Re-check with this RPC" where the wizard has one —
 * the two together, under the card whose sentence points at them.
 *
 * "No RPC endpoint is listed for this network. Enter one, then re-check."
 * stood here over nothing to enter it in and nothing to re-check with.
 */
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
// The app's own reset: a field is as wide as its column, padding included.
import '../../../app.css';
import type { AddTokenModel, AddTokenResult } from '../model';
import AddToken from './AddToken.svelte';

const NO_RPC = 'No RPC endpoint is listed for this network. Enter one, then re-check.';
const RECHECK = 'Re-check with this RPC';

type NetworkCard = Extract<AddTokenResult, { kind: 'network' }>;

const card = (more: Partial<NetworkCard>): NetworkCard => ({
	kind: 'network',
	mark: { ticker: 'SMP', badgeColor: 'var(--color-fg-subtle)', badgeHidden: true },
	name: 'Sample',
	facts: [
		{ label: 'Chain ID', value: '7777777' },
		{ label: 'Native token', value: 'SMP' }
	],
	...more
});

const sheet = (result: AddTokenResult): AddTokenModel => ({
	title: 'Add token',
	closeLabel: 'Close',
	tab: 'native',
	tabs: { erc20: 'ERC-20', native: 'Network' },
	fieldLabel: 'Network name or chain ID',
	fieldValue: 'sample',
	fieldPlaceholder: 'Search',
	result,
	cta: 'Add network',
	ctaDisabled: true
});

const RPC = {
	label: 'RPC URL',
	value: '',
	placeholder: 'https://…',
	recheck: RECHECK
};

function recheckButton(container: Element): HTMLButtonElement | undefined {
	return [...container.querySelectorAll('button')].find((button) =>
		button.textContent?.includes(RECHECK)
	);
}

describe('the add-token network tab — the RPC field and its re-check', () => {
	it('the no-RPC stop: the sentence, the field it asks for under it, and the re-check under that', async () => {
		const oncustomrpc = vi.fn();
		const onrecheck = vi.fn();
		const host = document.createElement('div');
		host.style.width = '288px';
		document.body.appendChild(host);
		const screen = render(AddToken, {
			target: host,
			props: { model: sheet(card({ note: NO_RPC, rpc: RPC })), oncustomrpc, onrecheck }
		});
		const reason = screen.container.querySelector('.reason') as HTMLElement;
		expect(reason.textContent).toBe(NO_RPC);
		const field = screen.container.querySelector<HTMLInputElement>('input[aria-label="RPC URL"]');
		expect(field).not.toBeNull();
		expect(screen.container.textContent).not.toContain('optional');
		const recheck = recheckButton(screen.container);
		expect(recheck).toBeDefined();

		// In reading order: the sentence, the field, the re-check, the CTA.
		const top = (el: Element) => el.getBoundingClientRect().top;
		const cta = screen.container.querySelector('.cta') as HTMLElement;
		expect(top(field!)).toBeGreaterThan(reason.getBoundingClientRect().bottom);
		expect(top(recheck!)).toBeGreaterThanOrEqual(field!.getBoundingClientRect().bottom);
		expect(top(cta)).toBeGreaterThanOrEqual(recheck!.getBoundingClientRect().bottom);
		// Nothing wider than the 320 px sheet's column.
		for (const el of [field!, recheck!]) {
			expect(el.getBoundingClientRect().right).toBeLessThanOrEqual(
				host.getBoundingClientRect().right + 0.5
			);
		}

		// Typing goes to the core as typed; the re-check asks for the check.
		field!.value = 'https://my.rpc.example';
		field!.dispatchEvent(new Event('input', { bubbles: true }));
		expect(oncustomrpc).toHaveBeenCalledWith('https://my.rpc.example');
		recheck!.click();
		expect(onrecheck).toHaveBeenCalledTimes(1);
		screen.unmount();
		host.remove();
	});

	it('what the core holds for the field is what it shows', async () => {
		const screen = render(AddToken, {
			props: {
				model: sheet(
					card({
						chip: { text: 'Compatible', tone: 'success' },
						rpc: { ...RPC, label: 'Custom RPC (optional)', value: 'https://my.rpc.example' }
					})
				)
			}
		});
		const field = screen.container.querySelector<HTMLInputElement>(
			'input[aria-label="Custom RPC (optional)"]'
		);
		expect(field?.value).toBe('https://my.rpc.example');
		expect(recheckButton(screen.container)).toBeDefined();
	});

	it('a refusal has no field — and so no re-check', async () => {
		const screen = render(AddToken, {
			props: {
				model: sheet(
					card({
						chip: { text: 'Not compatible', tone: 'error' },
						note: "This network has no P-256 verifier (RIP-7212 at 0x100), so Vela wallets can't work here."
					})
				)
			}
		});
		// The search field is the only one.
		expect(screen.container.querySelectorAll('input')).toHaveLength(1);
		expect(recheckButton(screen.container)).toBeUndefined();
		expect(screen.container.textContent).not.toContain(RECHECK);
		// The hint's hex is drawn as written: "0x100", never "0×100".
		expect(screen.container.querySelector('.reason')?.textContent).toContain('0x100');
	});
});
