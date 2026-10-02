/**
 * Who is asking is never cut short (spec 089).
 *
 * The 360 px side panel drew `app.uniswap.org.secure-wallet-session-verify-
 * login-account.localhost:5187` as "app.uniswap.org.se…": an ellipsis keeps a
 * host's START and drops its end, and the end is the registrable domain — the
 * one part that says who is really asking. The name and the host now wrap, and
 * every character is on screen, inside the header.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import SigningHeader from './SigningHeader.svelte';
import type { SigningModel } from '../model';

/** The side panel's width, less the sheet's own padding either side. */
const PANEL_PX = 360;
const PAD_PX = 24;
const LONG = 'app.uniswap.org.secure-wallet-session-verify-login-account.localhost:5187';

let host: HTMLDivElement | null = null;

afterEach(() => {
	host?.remove();
	host = null;
});

async function drawn(dapp: SigningModel['dapp']) {
	host = document.createElement('div');
	host.style.cssText = `width:${PANEL_PX}px;padding:0 ${PAD_PX}px;box-sizing:border-box;`;
	document.body.appendChild(host);
	render(SigningHeader, {
		target: host,
		props: {
			dapp,
			network: { name: 'Ethereum', dot: 'var(--color-fg-muted)' },
			onclose: () => {},
			closeLabel: 'Close'
		}
	});
	await tick();
	await document.fonts.ready;
	return { box: host.getBoundingClientRect() };
}

/** Every character of `el`'s text is laid out, and none of it past `right`. */
function wholly(el: HTMLElement, right: number) {
	expect(el.textContent).toBe(LONG);
	expect(el.scrollWidth, 'nothing scrolled out of the box').toBeLessThanOrEqual(el.clientWidth);
	const range = document.createRange();
	range.selectNodeContents(el);
	for (const rect of range.getClientRects()) expect(rect.right).toBeLessThanOrEqual(right + 0.5);
	expect(getComputedStyle(el).textOverflow).not.toBe('ellipsis');
}

describe('the sheet header names the whole origin (089)', () => {
	it('a long host that IS the name wraps, end and all', async () => {
		const { box } = await drawn({
			name: LONG,
			host: '',
			letter: 'A',
			tint: 'var(--color-fg-muted)'
		});
		wholly(document.querySelector<HTMLElement>('.name')!, box.right - PAD_PX);
	});

	it('a long host under a name wraps too', async () => {
		const { box } = await drawn({
			name: 'Uniswap',
			host: LONG,
			letter: 'U',
			tint: 'var(--color-fg-muted)'
		});
		wholly(document.querySelector<HTMLElement>('.host')!, box.right - PAD_PX);
	});

	it('an everyday host is one line, as before', async () => {
		await drawn({ name: 'app.uniswap.org', host: '', letter: 'A', tint: 'var(--color-fg-muted)' });
		const name = document.querySelector<HTMLElement>('.name')!;
		const range = document.createRange();
		range.selectNodeContents(name);
		expect(new Set([...range.getClientRects()].map((r) => Math.round(r.top))).size).toBe(1);
	});
});
