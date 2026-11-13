/**
 * The desktop dialog's keyboard contract (vis-2): focus moves in on open,
 * Tab stays inside, Escape closes the dialog on top and only it, and focus
 * returns to what opened it. Also (vis-13) a long body scrolls inside the
 * capped dialog instead of running out of it.
 */
import { createRawSnippet, tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';
import '$lib/tokens/tokens.css';
import Dialog from './Dialog.svelte';

const body = (html: string) => createRawSnippet(() => ({ render: () => html }));

function frame(): HTMLElement {
	const host = document.createElement('div');
	host.style.cssText = 'position: relative; width: 1280px; height: 700px; overflow: hidden;';
	document.body.appendChild(host);
	return host;
}

async function open(
	html = '<div><button type="button" class="a">A</button><button type="button" class="b">B</button></div>'
) {
	const host = frame();
	const opener = document.createElement('button');
	opener.textContent = 'open';
	document.body.appendChild(opener);
	opener.focus();
	const onclose = vi.fn();
	const screen = render(Dialog, {
		target: host,
		props: { title: 'Dialog', closeLabel: 'Close', onclose, children: body(html) }
	});
	await tick();
	const dialog = host.querySelector<HTMLElement>('[role="dialog"]')!;
	return { host, opener, onclose, screen, dialog };
}

describe('Dialog', () => {
	it('takes focus on open and gives it back on close', async () => {
		const { opener, screen, dialog, host } = await open();
		expect(dialog.contains(document.activeElement)).toBe(true);
		screen.unmount();
		await tick();
		expect(document.activeElement).toBe(opener);
		host.remove();
		opener.remove();
	});

	it('keeps Tab inside, both ways', async () => {
		const { dialog, host, opener, screen } = await open();
		const close = dialog.querySelector<HTMLButtonElement>('.close')!;
		const last = dialog.querySelector<HTMLButtonElement>('.b')!;
		const tab = (shiftKey: boolean) =>
			document.activeElement?.dispatchEvent(
				new KeyboardEvent('keydown', { key: 'Tab', shiftKey, bubbles: true, cancelable: true })
			);
		last.focus();
		tab(false);
		expect(document.activeElement).toBe(close);
		tab(true);
		expect(document.activeElement).toBe(last);
		screen.unmount();
		host.remove();
		opener.remove();
	});

	it('closes on Escape — the top one only', async () => {
		const under = await open();
		const over = await open();
		await userEvent.keyboard('{Escape}');
		expect(over.onclose).toHaveBeenCalledOnce();
		expect(under.onclose).not.toHaveBeenCalled();
		over.screen.unmount();
		await tick();
		await userEvent.keyboard('{Escape}');
		expect(under.onclose).toHaveBeenCalledOnce();
		under.screen.unmount();
		for (const v of [under, over]) {
			v.host.remove();
			v.opener.remove();
		}
	});

	it('scrolls a body taller than itself, inside its cap (vis-13)', async () => {
		const rows = Array.from({ length: 60 }, (_, i) => `<p>Row ${i}</p>`).join('');
		const { dialog, host, opener, screen } = await open(
			`<div>${rows}<button type="button">Reject</button></div>`
		);
		const content = dialog.querySelector<HTMLElement>('.content')!;
		expect(dialog.getBoundingClientRect().bottom).toBeLessThanOrEqual(
			host.getBoundingClientRect().bottom
		);
		expect(content.scrollHeight).toBeGreaterThan(content.clientHeight);
		screen.unmount();
		host.remove();
		opener.remove();
	});
});
