/**
 * Owner decision 2026-10-28, in a real browser: a network whose RPC is the
 * person's own offers "Only use my node", and says when that node is not
 * answering — on its row in the list, and beside the RPC field.
 */
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page } from 'vitest/browser';
import '$lib/tokens/tokens.css';
import type { NetworkDetailModel, NetworkRowModel } from '../model';
import NetworkDetailPanel from './NetworkDetailPanel.svelte';
import NetworkRow from './NetworkRow.svelte';

const mark = { letter: 'G', color: 'var(--color-fg-subtle)' };

const detail = (parts: Partial<NetworkDetailModel> = {}): NetworkDetailModel => ({
	title: 'Gnosis',
	subtitle: 'Chain 100 · xDAI',
	mark,
	name: 'Gnosis',
	note: 'Built-in network',
	badge: { tone: 'neutral', label: 'Online' },
	rpc: { id: 'rpc', label: 'RPC URL', value: 'https://my-node.example' },
	explorer: { id: 'explorer', label: 'Explorer', value: 'https://gnosisscan.io' },
	...parts
});

describe('"Only use my node"', () => {
	it('is a switch beside the RPC, and says what it does', async () => {
		const ononlymine = vi.fn();
		render(NetworkDetailPanel, {
			props: {
				detail: detail({
					onlyMine: {
						id: 'only-mine',
						title: 'Only use my node',
						subtitle: 'Nothing about your account goes to public nodes.',
						on: false
					}
				}),
				ononlymine
			}
		});
		const toggle = page.getByRole('switch', { name: /Only use my node/ });
		await expect.element(toggle).toHaveAttribute('aria-checked', 'false');
		await toggle.click();
		expect(ononlymine).toHaveBeenCalledWith(true);
	});

	it('is not offered without an RPC of the person’s own', () => {
		const screen = render(NetworkDetailPanel, { props: { detail: detail() } });
		expect(screen.container.querySelector('[role="switch"]')).toBeNull();
	});

	it('the node’s notice stands beside the field', async () => {
		render(NetworkDetailPanel, {
			props: {
				detail: detail({
					nodeCallout: {
						tone: 'warning',
						text: "Your node isn't answering — using public nodes for now"
					}
				})
			}
		});
		await expect
			.element(page.getByText("Your node isn't answering — using public nodes for now"))
			.toBeVisible();
	});
});

describe('the network row', () => {
	const row = (notice?: NetworkRowModel['notice']): NetworkRowModel => ({
		id: 'gnosis',
		mark,
		name: 'Gnosis',
		meta: 'Chain 100',
		...(notice === undefined ? {} : { notice })
	});

	it('reads amber while public nodes stand in, red when the node alone cannot be reached', async () => {
		const amber = render(NetworkRow, {
			props: { row: row({ tone: 'warn', text: "Your node isn't answering" }) }
		});
		const warn = amber.container.querySelector('[data-testid="network-node-notice"]')!;
		expect(warn.classList.contains('warn')).toBe(true);
		expect(getComputedStyle(warn).color).not.toBe(
			getComputedStyle(amber.container.querySelector('.meta')!).color
		);
		amber.unmount();

		const red = render(NetworkRow, {
			props: { row: row({ tone: 'error', text: "Can't reach your node for Gnosis" }) }
		});
		const error = red.container.querySelector('[data-testid="network-node-notice"]')!;
		expect(error.classList.contains('error')).toBe(true);
		expect(error.textContent).toBe("Can't reach your node for Gnosis");
		red.unmount();

		const none = render(NetworkRow, { props: { row: row() } });
		expect(none.container.querySelector('[data-testid="network-node-notice"]')).toBeNull();
	});
});
