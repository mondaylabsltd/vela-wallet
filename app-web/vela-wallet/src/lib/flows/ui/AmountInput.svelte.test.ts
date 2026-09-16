/**
 * The ⇄ denomination toggle (issue 197): the icon beneath the figure was
 * drawn on every send form and had nothing behind it — no handler on the
 * shells' side, and no read of the three `SendView.denom_toggle_*` fields
 * that say whether it should be there and whether it can act.
 *
 * A `.svelte.test.ts`: whether a real click reaches a real handler, and
 * whether a disabled control refuses it, is the browser's to answer.
 */
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import AmountInput from './AmountInput.svelte';

const BASE = { value: '0.5', fiat: '≈ $1,500.00', denomLabel: 'ETH' };

describe('AmountInput', () => {
	it('presses through to the core when the swap is on offer', async () => {
		const ondenom = vi.fn();
		const screen = render(AmountInput, {
			props: { ...BASE, denomToggle: { enabled: true }, ondenom }
		});
		const toggle = screen.container.querySelector('button.fiat');
		expect(toggle).not.toBeNull();
		expect((toggle as HTMLButtonElement).disabled).toBe(false);
		(toggle as HTMLButtonElement).click();
		expect(ondenom).toHaveBeenCalledTimes(1);
	});

	it('refuses the press, visibly, when the core says the swap would do nothing', async () => {
		const ondenom = vi.fn();
		const screen = render(AmountInput, {
			props: { ...BASE, denomToggle: { enabled: false }, ondenom }
		});
		const toggle = screen.container.querySelector('button.fiat') as HTMLButtonElement;
		expect(toggle.disabled).toBe(true);
		toggle.click();
		expect(ondenom).not.toHaveBeenCalled();
	});

	it('draws no control at all where the core offers no swap', () => {
		const screen = render(AmountInput, { props: BASE });
		expect(screen.container.querySelector('button.fiat')).toBeNull();
		// The other denomination is still worth reading — it is just not a door.
		expect(screen.container.querySelector('p.fiat')?.textContent).toBe('≈ $1,500.00');
	});

	it('names the button by the line it shows, not by the unit being left', () => {
		const screen = render(AmountInput, {
			props: { ...BASE, denomToggle: { enabled: true }, ondenom: () => {} }
		});
		const toggle = screen.container.querySelector('button.fiat') as HTMLButtonElement;
		expect(toggle.getAttribute('aria-label')).toBeNull();
		expect(toggle.textContent).toContain('≈ $1,500.00');
	});
});
