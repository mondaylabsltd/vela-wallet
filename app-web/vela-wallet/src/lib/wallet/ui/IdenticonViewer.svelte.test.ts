/**
 * The identicon viewer's desktop card (vis-2): it takes focus when it opens,
 * closes on Escape, and that Escape is its own — a dialog under it (the
 * account switcher) stays open.
 */
import { createRawSnippet, tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page, userEvent } from 'vitest/browser';
import '$lib/tokens/tokens.css';
import Dialog from '$lib/settings/ui/Dialog.svelte';
import IdenticonViewer from './IdenticonViewer.svelte';

const COPY = {
	title: 'Identicon',
	caption: 'A visual fingerprint of this address.',
	copyAddress: 'Copy address',
	copied: 'Copied',
	close: 'Close',
	a11yOpen: 'View identicon'
};
const ADDRESS = '0x031d7D57c99CAF891e1C250554691Fd12D84772b';

afterEach(async () => {
	await page.viewport(414, 896);
});

describe('IdenticonViewer on the desktop', () => {
	it('takes focus, and Escape closes it alone', async () => {
		await page.viewport(1440, 900);
		const host = document.createElement('div');
		host.style.cssText = 'position: relative; width: 1440px; height: 900px;';
		document.body.appendChild(host);
		const ondialogclose = vi.fn();
		render(Dialog, {
			target: host,
			props: {
				title: 'Accounts',
				closeLabel: 'Close',
				onclose: ondialogclose,
				children: createRawSnippet(() => ({ render: () => '<p>rows</p>' }))
			}
		});
		const onClose = vi.fn();
		const viewer = render(IdenticonViewer, {
			props: { copy: COPY, address: ADDRESS, identiconSvg: '', onClose }
		});
		await tick();
		const card = viewer.container.querySelector('[role="dialog"]') as HTMLElement;
		expect(card.contains(document.activeElement)).toBe(true);
		await userEvent.keyboard('{Escape}');
		expect(onClose).toHaveBeenCalledOnce();
		expect(ondialogclose).not.toHaveBeenCalled();
		host.remove();
	});
});

describe('IdenticonViewer copy', () => {
	// home-19: copying an address moves nothing, so its button wears the
	// neutral ink — the accent is only for the action that moves value.
	it('draws Copy address in the neutral primary, never the accent', async () => {
		const viewer = render(IdenticonViewer, {
			props: { copy: COPY, address: ADDRESS, identiconSvg: '', onClose: () => {} }
		});
		await tick();
		const copy = Array.from(viewer.container.querySelectorAll('button')).find(
			(b) => b.textContent?.trim() === COPY.copyAddress
		) as HTMLButtonElement;
		expect(copy.classList.contains('primary')).toBe(true);
		expect(copy.classList.contains('value')).toBe(false);
		const fill = getComputedStyle(copy).backgroundColor;
		const probe = document.createElement('span');
		probe.style.color = 'var(--color-accent-base)';
		document.body.appendChild(probe);
		expect(fill).not.toBe(getComputedStyle(probe).color);
		probe.remove();
	});
});
