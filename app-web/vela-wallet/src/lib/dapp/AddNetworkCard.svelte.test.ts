/**
 * dapp-13: "Unable to verify" was a dead end for a node the check could not
 * reach. The card offers adding it unverified — behind its warning, and only
 * on an explicit confirmation.
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import AddNetworkCard from './AddNetworkCard.svelte';
import type { AddNetworkCard as Card } from './add-network';

const card: Card = {
	title: 'Add Network',
	lead: '127.0.0.1:8941 asks to add a network',
	rows: [{ label: 'Chain ID', value: '1337' }],
	fromSite: null,
	pill: { tone: 'warn', label: 'Unable to verify', dot: true },
	checksTitle: null,
	checks: [],
	note: 'Vela couldn’t reach 127.0.0.1:8941 to check it.',
	add: null,
	unverified: {
		open: 'Add without verifying',
		warning: 'Money sent to your Vela address there could be stuck.',
		confirm: 'Add unverified network'
	},
	retry: 'Retry',
	setupTool: null,
	dismiss: 'Cancel'
};

const buttons = (root: HTMLElement) =>
	[...root.querySelectorAll('button')].map((b) => b.textContent?.trim() ?? '');
const press = (root: HTMLElement, label: string) =>
	[...root.querySelectorAll('button')].find((b) => b.textContent?.trim() === label)?.click();

describe('the add-network card, unable to verify', () => {
	it('adds unverified only after the warning and its confirmation', async () => {
		const onaddunverified = vi.fn();
		const onadd = vi.fn();
		const screen = render(AddNetworkCard, {
			props: {
				card,
				onadd,
				onaddunverified,
				onretry: () => {},
				ondismiss: () => {},
				onsetup: () => {}
			}
		});
		const root = screen.container as HTMLElement;
		await tick();
		expect(buttons(root)).toContain('Retry');
		expect(buttons(root)).toContain('Add without verifying');
		expect(root.textContent).not.toContain(card.unverified!.warning);

		press(root, 'Add without verifying');
		await tick();
		expect(root.textContent).toContain(card.unverified!.warning);
		expect(onaddunverified).not.toHaveBeenCalled();

		press(root, 'Add unverified network');
		await tick();
		expect(onaddunverified).toHaveBeenCalledTimes(1);
		expect(onadd).not.toHaveBeenCalled();
		await screen.unmount();
	});

	it('offers nothing unverified where the core does not allow it', async () => {
		const screen = render(AddNetworkCard, {
			props: {
				card: { ...card, unverified: null },
				onadd: () => {},
				onaddunverified: () => {},
				onretry: () => {},
				ondismiss: () => {},
				onsetup: () => {}
			}
		});
		await tick();
		expect(buttons(screen.container as HTMLElement)).not.toContain('Add without verifying');
		await screen.unmount();
	});
});
