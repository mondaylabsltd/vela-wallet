/**
 * Spec 090: the "include network" switch under the receive code. The sheet
 * draws exactly what the model (the core's view) says — the switch's
 * position, the hint only while it is on — and a press asks for the other
 * position rather than flipping anything itself.
 */
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import ReceiveQr from './ReceiveQr.svelte';
import type { ReceiveQrModel } from '../model';

const HINT = "Some wallets can't read this code. If theirs can't, switch it off.";

function model(network: ReceiveQrModel['network']): ReceiveQrModel {
	return {
		title: 'Use this address to receive assets on Gnosis',
		closeLabel: 'Close',
		account: {
			name: 'My Wallet',
			identiconSvg: '<svg/>',
			lines: ['0x14fB1fB21751E29F7Ec', '48dC450017552E3D1eA5c'],
			copyLabel: 'Copy address'
		},
		centre: { ticker: 'XDAI', badgeColor: '#04795b' },
		network,
		warning: 'Supported networks only — transfers on other networks may be lost.',
		saveImage: 'Save image',
		viewOnExplorer: 'View on explorer'
	};
}

const switchOf = (el: Element) => el.querySelector<HTMLButtonElement>('[role="switch"]');

describe('ReceiveQr — the include-network switch', () => {
	it('off: the switch reads unchecked and no hint is drawn', () => {
		const { container } = render(ReceiveQr, {
			props: { model: model({ label: 'Include network', on: false }) }
		});
		expect(switchOf(container)?.getAttribute('aria-checked')).toBe('false');
		expect(switchOf(container)?.textContent).toContain('Include network');
		expect(container.querySelector('.hint')).toBeNull();
	});

	it('on: checked, with the calm hint under it in the subtle ink', () => {
		const { container } = render(ReceiveQr, {
			props: { model: model({ label: 'Include network', on: true, hint: HINT }) }
		});
		expect(switchOf(container)?.getAttribute('aria-checked')).toBe('true');
		const hint = container.querySelector<HTMLElement>('.hint');
		expect(hint?.textContent).toBe(HINT);
		const subtle = getComputedStyle(document.documentElement)
			.getPropertyValue('--color-fg-subtle')
			.trim();
		const probe = document.createElement('span');
		probe.style.color = subtle;
		document.body.appendChild(probe);
		expect(getComputedStyle(hint!).color).toBe(getComputedStyle(probe).color);
		probe.remove();
	});

	it('a press asks for the other position', () => {
		const onincludenetwork = vi.fn();
		const { container } = render(ReceiveQr, {
			props: { model: model({ label: 'Include network', on: false }), onincludenetwork }
		});
		switchOf(container)?.click();
		expect(onincludenetwork).toHaveBeenCalledWith(true);
	});

	it('no switch where the core offers none', () => {
		const { container } = render(ReceiveQr, { props: { model: model(undefined) } });
		expect(switchOf(container)).toBeNull();
	});
});
