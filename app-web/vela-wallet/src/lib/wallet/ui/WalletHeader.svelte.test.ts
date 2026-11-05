/**
 * home-19: the header's name opens the account switcher; its address copies
 * the whole address and says "Copied" — it used to open the switcher too,
 * which surprised everyone who tapped it to copy it. A refused clipboard is
 * never a silent tap: the viewer shows the address whole, for the taking.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';
import '$lib/tokens/tokens.css';
import { identiconViewer } from '../identicon-viewer.svelte';
import WalletHeader from './WalletHeader.svelte';

const ADDRESS = '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c';
const HEADER = {
	name: 'Alice',
	addressDisplay: '0x14fB1f…D1eA5c',
	addressFull: ADDRESS,
	identiconSvg: '<svg viewBox="0 0 1 1"></svg>'
};

function words(): void {
	identiconViewer.copyLabel = 'Copy address';
	identiconViewer.copiedLabel = 'Copied';
}

afterEach(() => {
	vi.restoreAllMocks();
	identiconViewer.close();
	identiconViewer.copyLabel = '';
	identiconViewer.copiedLabel = '';
});

describe('WalletHeader', () => {
	it('opens the switcher from the name, not from the address', async () => {
		words();
		const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
		const onclick = vi.fn();
		const screen = render(WalletHeader, { props: { header: HEADER, onclick } });
		(screen.container.querySelector('.name-row') as HTMLButtonElement).click();
		expect(onclick).toHaveBeenCalledOnce();

		const address = screen.container.querySelector(
			'[data-testid="header-address"]'
		) as HTMLButtonElement;
		expect(address.getAttribute('aria-haspopup')).toBeNull();
		expect(address.getAttribute('aria-label')).toBe('Copy address 0x14fB1f…D1eA5c');
		address.click();
		await vi.waitFor(() => expect(writeText).toHaveBeenCalledWith(ADDRESS));
		expect(onclick).toHaveBeenCalledOnce();
	});

	/*
	 * r14-1: on the iPhone (the Safari extension's wallet tab) the switcher
	 * never opened, and WebKit showed the header as a group, not a button.
	 * WebKit maps a `<button aria-haspopup>` to a pop-up button, which iOS
	 * exposes with no button trait — VoiceOver and automation saw "SafariE2E,
	 * group". It is a plain button, named by the account, and a press anywhere
	 * on it — the name, the chevron — opens the switcher.
	 */
	it('is a plain button named by the account, and the name and the chevron both open the switcher', async () => {
		const onclick = vi.fn();
		const screen = render(WalletHeader, {
			props: { header: { ...HEADER, name: 'A rather long account name, kept' }, onclick }
		});
		const button = screen.getByRole('button', { name: 'A rather long account name, kept' });
		const element = button.element() as HTMLButtonElement;
		expect(element.tagName).toBe('BUTTON');
		expect(element.type).toBe('button');
		expect(element.getAttribute('role')).toBeNull();
		expect(element.getAttribute('aria-haspopup')).toBeNull();

		// A real press on each part, at the part's own centre.
		await userEvent.click(element.querySelector('.name') as HTMLElement);
		expect(onclick).toHaveBeenCalledTimes(1);
		await userEvent.click(element.querySelector('.chevron') as HTMLElement);
		expect(onclick).toHaveBeenCalledTimes(2);
	});

	it('says "Copied" once the address is on the clipboard, then goes back', async () => {
		words();
		vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
		vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
		try {
			const screen = render(WalletHeader, { props: { header: HEADER, onclick: () => {} } });
			const address = screen.container.querySelector(
				'[data-testid="header-address"]'
			) as HTMLButtonElement;
			const status = screen.container.querySelector('[role="status"]') as HTMLElement;
			expect(status.textContent).toBe('');
			address.click();
			await vi.waitFor(() => expect(status.textContent).toBe('Copied'));
			expect(address.textContent).toContain('Copied');
			vi.advanceTimersByTime(1600);
			await tick();
			expect(status.textContent).toBe('');
			expect(address.textContent).not.toContain('Copied');
		} finally {
			vi.useRealTimers();
		}
	});

	it('shows the address whole in the viewer when the clipboard refuses', async () => {
		words();
		vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(new Error('denied'));
		const screen = render(WalletHeader, { props: { header: HEADER, onclick: () => {} } });
		(screen.container.querySelector('[data-testid="header-address"]') as HTMLElement).click();
		await vi.waitFor(() =>
			expect(identiconViewer.current).toEqual({
				address: ADDRESS,
				identiconSvg: HEADER.identiconSvg
			})
		);
		expect(screen.container.querySelector('[role="status"]')?.textContent).toBe('');
	});

	it('is a picture in the gallery and before the session answers', () => {
		const gallery = render(WalletHeader, { props: { header: HEADER } });
		expect(gallery.container.querySelector('[data-testid="header-address"]')).toBeNull();
		expect(gallery.container.textContent).toContain('0x14fB1f…D1eA5c');
		const blank = render(WalletHeader, {
			props: { header: { ...HEADER, addressDisplay: '', addressFull: '' }, onclick: () => {} }
		});
		expect(blank.container.querySelector('[data-testid="header-address"]')).toBeNull();
	});
});
