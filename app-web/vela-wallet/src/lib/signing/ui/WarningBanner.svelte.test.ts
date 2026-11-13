/**
 * dapp-15: a could-not-check line about the network offers "Check again", and
 * while the new check is out it stays — marked stale — with "Checking again…".
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import WarningBanner from './WarningBanner.svelte';

const TEXT = 'Vela couldn’t check what this transaction does. Review it before you sign.';

describe('the could-not-check line', () => {
	it('offers Check again, and says it is checking again over the stale line', async () => {
		const onrecheck = vi.fn();
		const screen = render(WarningBanner, {
			props: {
				tone: 'caution',
				text: TEXT,
				recheck: { label: 'Check again', checking: false },
				onrecheck
			}
		});
		await tick();
		const root = screen.container as HTMLElement;
		const button = [...root.querySelectorAll('button')].find(
			(b) => b.textContent?.trim() === 'Check again'
		);
		expect(button).toBeTruthy();
		button?.click();
		expect(onrecheck).toHaveBeenCalledTimes(1);

		await screen.rerender({ recheck: { label: 'Checking again…', checking: true } });
		await tick();
		expect(root.querySelector('button')).toBeNull();
		expect(root.querySelector('[role="status"]')?.textContent).toBe('Checking again…');
		expect(root.querySelector('.stale')).not.toBeNull();
		// Still said: the line is not removed while it is being replaced.
		expect(root.textContent).toContain(TEXT);
		await screen.unmount();
	});

	it('without a recheck, is the plain line it always was', async () => {
		const screen = render(WarningBanner, { props: { tone: 'caution', text: TEXT } });
		await tick();
		expect(screen.container.querySelector('button')).toBeNull();
		expect(screen.container.querySelector('.stale')).toBeNull();
		await screen.unmount();
	});

	it('names nothing `.said` — that is the verdict place it is drawn in, and one name is one element', async () => {
		// The place's said cell is how the sheet (and every e2e) finds what
		// was said; a banner of the same name inside it made that two elements.
		for (const recheck of [undefined, { label: 'Checking again…', checking: true }]) {
			const screen = render(WarningBanner, { props: { tone: 'caution', text: TEXT, recheck } });
			await tick();
			expect(screen.container.querySelector('.said')).toBeNull();
			expect(screen.container.textContent).toContain(TEXT);
			await screen.unmount();
		}
	});
});
