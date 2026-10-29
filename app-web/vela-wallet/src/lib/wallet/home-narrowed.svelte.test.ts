/**
 * The phone home says when it is narrowed to one network (the owner's
 * report: the side panel listed BNB Chain's three tokens under the whole
 * wallet's total, and nothing on the home said the list was narrowed — the
 * pill that did was taken off the home on 2026-08-26). The line names the
 * network and holds the way back; with no filter there is no line at all.
 */
import '$lib/tokens/tokens.css';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { words } from '$lib/settings/__fixtures__/words';
import { buildMobileState } from './fixtures';
import type { WalletMessages } from './messages';
import WalletHome from './WalletHome.svelte';

const m = words() as unknown as WalletMessages;
const identicon = (seed: string) => `<svg viewBox="0 0 8 8" data-seed="${seed}"></svg>`;
const model = () => buildMobileState('h1', m, identicon);

describe('the phone home, narrowed to one network', () => {
	it('says which network, and the way back to every one', async () => {
		const onshowall = vi.fn();
		const screen = render(WalletHome, {
			props: {
				model: model(),
				narrowed: { label: 'Showing BNB Chain only', action: 'Show every chain' },
				onshowall
			}
		});
		const line = screen.getByTestId('home-narrowed');
		await expect.element(line).toHaveTextContent('Showing BNB Chain only');
		await expect.element(line).toHaveAttribute('role', 'status');
		await screen.getByRole('button', { name: 'Show every chain' }).click();
		expect(onshowall).toHaveBeenCalledTimes(1);
		screen.unmount();
	});

	it('draws no line while every network is shown', async () => {
		const screen = render(WalletHome, { props: { model: model() } });
		expect(screen.container.querySelector('[data-testid="home-narrowed"]')).toBeNull();
		screen.unmount();
	});
});
