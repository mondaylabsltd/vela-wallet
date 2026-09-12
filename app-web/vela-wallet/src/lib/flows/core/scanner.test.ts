/**
 * Every refusal has its own name (spec 028 T424).
 *
 * A scanner's failure modes all look identical to a person — a black frame —
 * and each has a different thing to do about it. These pin the classification,
 * because the value of this module is not that it decodes; it is that it says
 * WHY when it cannot.
 */
import { describe, expect, it, vi } from 'vitest';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import { Scanner, scanNotice, type ScanStatus } from './scanner.svelte';

/** Drive `start()` with a `getUserMedia` that rejects the way a browser does. */
async function statusAfterRejecting(name: string): Promise<string> {
	const scanner = new Scanner();
	const error = Object.assign(new Error(name), { name });
	vi.stubGlobal('navigator', { mediaDevices: { getUserMedia: () => Promise.reject(error) } });
	vi.stubGlobal('window', { isSecureContext: true });
	await scanner.start({} as HTMLVideoElement);
	vi.unstubAllGlobals();
	return scanner.status;
}

describe('a camera that will not open says which kind of no it is', () => {
	it('a refusal is a refusal — this time, or once and remembered', async () => {
		// The browser reports both the same way, and for a person they are the
		// same instruction: change it in the site settings.
		expect(await statusAfterRejecting('NotAllowedError')).toBe('denied');
		expect(await statusAfterRejecting('SecurityError')).toBe('denied');
	});

	it('no camera is not a refusal — most desktops are simply like this', async () => {
		expect(await statusAfterRejecting('NotFoundError')).toBe('absent');
		expect(await statusAfterRejecting('OverconstrainedError')).toBe('absent');
	});

	it('anything else is "unavailable", never silence', async () => {
		expect(await statusAfterRejecting('AbortError')).toBe('unavailable');
	});

	it('separates "no camera API" from "not on HTTPS", which look the same', async () => {
		// `getUserMedia` is simply undefined off a secure origin, so the symptom
		// is identical and the fix is not: one needs a device, the other a URL.
		const insecure = new Scanner();
		vi.stubGlobal('navigator', { mediaDevices: undefined });
		vi.stubGlobal('window', { isSecureContext: false });
		await insecure.start({} as HTMLVideoElement);
		expect(insecure.status).toBe('insecure');

		const noCamera = new Scanner();
		vi.stubGlobal('window', { isSecureContext: true });
		await noCamera.start({} as HTMLVideoElement);
		expect(noCamera.status).toBe('absent');
		vi.unstubAllGlobals();
	});

	it('never asks when asking is impossible', async () => {
		// Checked BEFORE `getUserMedia`, so a device without a camera never
		// triggers a permission prompt someone then has to dismiss.
		vi.stubGlobal('navigator', { mediaDevices: undefined });
		vi.stubGlobal('window', { isSecureContext: true });
		expect(Scanner.supported()).toBe(false);
		vi.unstubAllGlobals();
	});
});

describe('the surface says which no it was', () => {
	const m = resolveWalletFlowMessages('en');
	const notice = (status: ScanStatus, extra: { nothingFound?: boolean; unusable?: boolean } = {}) =>
		scanNotice(
			{ status, nothingFound: extra.nothingFound ?? false, unusable: extra.unusable ?? false },
			m
		);

	it('gives every refusal its own sentence', () => {
		const sentences = (['denied', 'absent', 'insecure', 'unavailable'] as const).map((status) =>
			notice(status)
		);
		for (const sentence of sentences) expect(sentence?.trim()).toBeTruthy();
		// Four states, four DIFFERENT things to do. One sentence reused across
		// two of them is the dead viewfinder wearing words.
		expect(new Set(sentences).size).toBe(4);
	});

	it('says nothing while there is nothing wrong', () => {
		// The hint under the frame already says "point the camera at a code";
		// overwriting it with a status would be noise.
		for (const status of ['idle', 'starting', 'live'] as const) {
			expect(notice(status)).toBeUndefined();
		}
	});

	it('a picked image with no code in it is not a camera problem', () => {
		expect(notice('idle', { nothingFound: true })).toBe(m['componentsUi.scanner.noQrFoundMsg']);
	});

	it('a code that was READ and cannot be used never says "no QR found"', () => {
		// The lie this prevents: a QR plainly in frame, decoded, reported as
		// missing. It outranks every other notice for that reason.
		expect(notice('live', { unusable: true, nothingFound: true })).toBe(m['home.invalidQrTitle']);
	});
});
