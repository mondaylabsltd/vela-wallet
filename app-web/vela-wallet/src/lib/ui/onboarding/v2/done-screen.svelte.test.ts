/**
 * The Done screen's artwork — the first a person ever sees of their wallet —
 * opens the identicon viewer on the new address, like every artwork drawn
 * from an address (founder call, 2026-09-05). The address line beside it
 * stays its own copy button: neither control holds the other.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { identiconViewer } from '$lib/wallet/identicon-viewer.svelte';

const ADDRESS = '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c';

vi.mock('$lib/onboarding/core/wasm-client', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/onboarding/core/wasm-client')>()),
	identiconNormalizeSeed: (a: string) => a.toLowerCase(),
	identiconSvgCircular: (seed: string) => `<svg data-seed="${seed}"></svg>`
}));

const { default: DoneScreen } = await import('./DoneScreen.svelte');

afterEach(() => {
	identiconViewer.close();
	identiconViewer.openLabel = '';
});

describe('the Done screen', () => {
	it('opens the viewer on the new address from its artwork', async () => {
		identiconViewer.openLabel = 'View identicon';
		const screen = render(DoneScreen, {
			props: {
				address: ADDRESS,
				walletName: 'Alice',
				keys: [],
				strings: (key: string) => key,
				onEnter: () => {}
			}
		});
		await tick();
		const art = screen.container.querySelector<HTMLButtonElement>('.identity button.identicon');
		expect(art?.getAttribute('aria-label')).toBe('View identicon');
		art?.click();
		expect(identiconViewer.current).toEqual({
			address: ADDRESS,
			identiconSvg: `<svg data-seed="${ADDRESS.toLowerCase()}"></svg>`
		});
		expect(screen.container.querySelector('button button')).toBeNull();
	});
});
