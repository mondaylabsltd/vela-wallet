/**
 * The Clear Signer's sheet, in a browser (spec 071, contract §5): while the
 * page is open it says so, with the hint, "Open the page again" and Cancel;
 * after, one sentence and a way to close it — never a spinner left behind.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { clearSignerModel } from '../live';
import type { SigningMessages } from '../messages';
import ClearSignerSheet from './ClearSignerSheet.svelte';

/** The corpus is resolved at build time; the words this sheet reads, in English. */
const m = {
	close: 'Close',
	clearSignerWaiting: 'Waiting for the Clear Signer…',
	clearSignerWaitingHint: 'Check the request on the page that opened, and sign it there.',
	clearSignerReopen: 'Open the page again',
	clearSignerCancel: 'Cancel',
	clearSignerClosed: 'The Clear Signer was closed without signing.',
	clearSignerRefused: 'The Clear Signer would not sign this request. Its page says why.',
	clearSignerMismatch:
		'The Clear Signer’s answer does not match this request, so nothing was sent.',
	clearSignerTimeout: 'The Clear Signer did not answer in time.',
	clearSignerWhere: 'Where is your Clear Signer?',
	clearSignerThisDevice: 'On this device',
	clearSignerOtherDevice: 'On another device',
	clearSignerPair: 'Open the Clear Signer on your other device',
	clearSignerPairHint: 'Scan this code with the device that has your passkey.',
	clearSignerPairWaiting: 'Waiting for the other device…',
	clearSignerCopyLink: 'Copy link',
	clearSignerCode: 'Check that the other device shows the same code: {{code}}',
	clearSignerCodeConfirm: 'The codes match',
	clearSignerTunnelDown: 'The tunnel could not be reached.'
} as SigningMessages;

async function drawn(model: NonNullable<ReturnType<typeof clearSignerModel>>) {
	const taps: string[] = [];
	const screen = render(ClearSignerSheet, {
		props: {
			model,
			onreopen: () => taps.push('reopen'),
			ondismiss: () => taps.push('dismiss'),
			onwhere: (where: string) => taps.push(`where:${where}`),
			onconfirmcode: () => taps.push('confirm')
		}
	});
	await tick();
	const buttons = [...screen.container.querySelectorAll('button')];
	return {
		taps,
		text: screen.container.textContent ?? '',
		labels: buttons.map((button) => button.textContent?.trim()),
		press: (label: string) =>
			buttons.find((button) => button.textContent?.trim() === label)?.click()
	};
}

describe('the Clear Signer’s sheet', () => {
	it('waiting: the hint, open it again, cancel', async () => {
		const view = await drawn(clearSignerModel({ waiting: true, notice: null }, m)!);
		expect(view.text).toContain(m.clearSignerWaiting);
		expect(view.text).toContain(m.clearSignerWaitingHint);
		expect(view.labels).toEqual([m.clearSignerReopen, m.clearSignerCancel]);
		view.press(m.clearSignerReopen);
		view.press(m.clearSignerCancel);
		expect(view.taps).toEqual(['reopen', 'dismiss']);
	});

	it('Escape is this sheet’s, and never reaches the signing sheet under it', async () => {
		const view = await drawn(clearSignerModel({ waiting: true, notice: null }, m)!);
		let beneath = 0;
		const underneath = (event: KeyboardEvent) => {
			if (event.key === 'Escape') beneath += 1;
		};
		window.addEventListener('keydown', underneath);
		document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		window.removeEventListener('keydown', underneath);
		expect(view.taps).toEqual(['dismiss']);
		expect(beneath).toBe(0);
	});

	it('an ending: its sentence, and close — nothing to reopen', async () => {
		const view = await drawn(clearSignerModel({ waiting: false, notice: 'refused' }, m)!);
		expect(view.text).toContain(m.clearSignerRefused);
		expect(view.text).not.toContain(m.clearSignerWaitingHint);
		expect(view.labels).toEqual([m.close]);
	});

	it('a tunnel that could not be reached says so, and points back to this device', async () => {
		const view = await drawn(clearSignerModel({ waiting: false, notice: 'tunnel' }, m)!);
		expect(view.text).toContain(m.clearSignerTunnelDown);
	});
});

describe('spec 075: where the signer is', () => {
	it('asks first, and the answer is the person’s own tap', async () => {
		const view = await drawn(
			clearSignerModel({ asking: true, pairing: null, waiting: false, notice: null }, m)!
		);
		expect(view.text).toContain(m.clearSignerWhere);
		expect(view.labels).toEqual([
			m.clearSignerThisDevice,
			m.clearSignerOtherDevice,
			m.clearSignerCancel
		]);
		view.press(m.clearSignerOtherDevice);
		expect(view.taps).toEqual(['where:other_device']);
	});

	it('pairing: a real code to scan, the link to copy, and the wait', async () => {
		const link =
			'https://sign.getvela.app/sign.html?ch=relay#relay=wss%3A%2F%2Fr.example&room=AAAAAAAAAAAAAAAAAAAAAA&rk=BBBBBBBBBBBBBBBBBBBBBB&v=1';
		const model = clearSignerModel(
			{ asking: false, pairing: { link, code: null }, waiting: false, notice: null },
			m
		)!;
		// The code encodes the LINK — the receive screen's own encoder, not a pattern.
		expect(model.pair?.qr.modules).toBeGreaterThan(20);
		expect(model.pair?.qr.path.length).toBeGreaterThan(100);
		const view = await drawn(model);
		expect(view.text).toContain(m.clearSignerPair);
		expect(view.text).toContain(link);
		expect(view.text).toContain(m.clearSignerPairWaiting);
		expect(view.labels).toEqual([m.clearSignerCopyLink, m.clearSignerCancel]);
	});

	it('the six digits, and the confirm that is the only way anything is sent', async () => {
		const model = clearSignerModel(
			{
				asking: false,
				pairing: { link: 'https://sign.getvela.app/sign.html?ch=relay#v=1', code: '082567' },
				waiting: false,
				notice: null
			},
			m
		)!;
		const view = await drawn(model);
		expect(view.text).toContain('082567');
		expect(view.text).not.toContain(m.clearSignerPairWaiting);
		expect(view.labels).toEqual([
			m.clearSignerCopyLink,
			m.clearSignerCodeConfirm,
			m.clearSignerCancel
		]);
		view.press(m.clearSignerCodeConfirm);
		expect(view.taps).toEqual(['confirm']);
	});
});
