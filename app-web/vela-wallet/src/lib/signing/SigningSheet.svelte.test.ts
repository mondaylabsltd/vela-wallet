/**
 * The signing sheet in a browser (spec 079).
 *
 * US1 — it closes only through its ✕: the owner lost a dApp's request to a
 * stray touch ("除非用户明确关掉，不应该很容易误操作，比如下滑就关掉了"), and the
 * page then had to ask again.
 */
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import SigningSheet from './SigningSheet.svelte';
import type { SigningModel } from './model';

const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function model(over: Partial<SigningModel> = {}): SigningModel {
	return {
		id: 'cs1',
		dapp: { name: 'app.example', host: '', letter: 'A', tint: 'var(--color-fg-muted)' },
		network: { name: 'Gnosis', dot: 'var(--color-fg-muted)' },
		blocks: [{ kind: 'intent', text: 'Send 1 xDAI', tone: 'neutral' }],
		tech: {
			title: 'Details',
			params: [],
			identities: [],
			copyLabel: 'Copy',
			explorerLabel: 'Explorer'
		},
		techOpen: false,
		fee: { kind: 'hidden' },
		signer: { label: 'Signing account', name: 'My Wallet', identiconSvg: '<svg></svg>' },
		confirm: { hint: 'Slide to confirm', action: 'Send', enabled: true },
		closeLabel: 'Close',
		panelTitle: 'Signature request',
		...over
	};
}

async function drawn(props: { model?: SigningModel; dismissible?: boolean }) {
	const onclose = vi.fn();
	const screen = render(SigningSheet, {
		props: {
			model: props.model ?? model(),
			dismissible: props.dismissible ?? true,
			onclose
		}
	});
	await tick();
	await pause(300); // past the entry animation
	const root = document.body;
	const sheet = root.querySelector<HTMLElement>('[role="dialog"]')!;
	return {
		screen,
		onclose,
		sheet,
		scrim: root.querySelector<HTMLElement>('.scrim')!,
		close: sheet.querySelector<HTMLButtonElement>('header.header .close')
	};
}

describe('the signing sheet closes only on its ✕', () => {
	it('the scrim and Escape leave it open, and nothing is answered', async () => {
		const view = await drawn({});
		view.scrim.click();
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		await pause(600);
		expect(view.onclose).not.toHaveBeenCalled();
		expect(view.sheet.isConnected).toBe(true);
		await view.screen.unmount();
	});

	it('its header ✕, named, closes it once — the host’s refusal', async () => {
		const view = await drawn({});
		expect(view.close).not.toBeNull();
		expect(view.close!.getAttribute('aria-label')).toBe('Close');
		view.close!.click();
		view.close!.click();
		await vi.waitFor(() => expect(view.onclose).toHaveBeenCalledOnce(), { timeout: 1500 });
		await pause(300);
		expect(view.onclose).toHaveBeenCalledOnce();
		await view.screen.unmount();
	});

	it('while the signature is in flight the ✕ is drawn shut', async () => {
		const view = await drawn({ dismissible: false });
		expect(view.close!.disabled).toBe(true);
		view.close!.click();
		await pause(600);
		expect(view.onclose).not.toHaveBeenCalled();
		await view.screen.unmount();
	});

	it('a refused request offers one way out — its labelled Close, no second ✕', async () => {
		const view = await drawn({ model: model({ dismissOnly: 'Close' }) });
		expect(view.close).toBeNull();
		await view.screen.unmount();
	});
});
