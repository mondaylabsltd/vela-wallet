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
import { boardSession, Scanner, scanNotice, type ScanStatus } from './scanner.svelte';

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

/**
 * PR 3 note 7 — PRIVACY. A gallery sweep on the desktop walked onto its
 * scanner state, the real camera started, and a frame of the person at the
 * machine was captured. On the web the same door stood open: the Explore
 * boards' scan button reached `getUserMedia` from a gallery page.
 *
 * A board never opens a camera. The gate is where the camera is started —
 * the one place — and it is decided by where the page lives, so a board
 * cannot forget to say what it is.
 */
describe('a board never opens a camera', () => {
	/** Every camera door, each counting the times it is walked through. */
	function watchedCamera() {
		const getUserMedia = vi.fn(() => Promise.reject(new Error('the camera was asked for')));
		const enumerateDevices = vi.fn(() => Promise.resolve([]));
		let touched = 0;
		const navigator = {
			get mediaDevices() {
				touched += 1;
				return { getUserMedia, enumerateDevices };
			}
		};
		return { navigator, getUserMedia, enumerateDevices, touched: () => touched };
	}

	async function startedAt(pathname: string) {
		const camera = watchedCamera();
		vi.stubGlobal('navigator', camera.navigator);
		vi.stubGlobal('window', { isSecureContext: true });
		vi.stubGlobal('location', { pathname });
		const scanner = new Scanner();
		await scanner.start({} as HTMLVideoElement);
		// The tools a person may still press on the board.
		await scanner.flip();
		await scanner.toggleTorch();
		const status = scanner.status;
		vi.unstubAllGlobals();
		return { status, camera };
	}

	it.each([
		'/dev/gallery',
		'/dev/gallery/',
		'/en/gallery',
		'/en/gallery/e1',
		'/zh/gallery/s1',
		'/zh-HK/gallery/ds1',
		'/en/gallery/sd2e'
	])('%s: the fixture frame, and the camera API is not so much as looked at', async (pathname) => {
		expect(boardSession(pathname)).toBe(true);
		const { status, camera } = await startedAt(pathname);
		expect(status).toBe('fixture');
		expect(camera.getUserMedia).not.toHaveBeenCalled();
		expect(camera.enumerateDevices).not.toHaveBeenCalled();
		// Not even `navigator.mediaDevices` is read: nothing can prompt.
		expect(camera.touched()).toBe(0);
	});

	it.each(['/en/wallet', '/zh/wallet', '/en', '/en/settings', '/en/request', '/en/parallel'])(
		'%s is the app itself: its scanner asks for the camera as it always did',
		async (pathname) => {
			expect(boardSession(pathname)).toBe(false);
			const { status, camera } = await startedAt(pathname);
			expect(camera.getUserMedia).toHaveBeenCalledTimes(1);
			// (This camera refuses, so the scan says so — never the fixture.)
			expect(status).not.toBe('fixture');
		}
	);

	it('a page that only mentions a gallery is not one', () => {
		for (const pathname of ['/en/wallet/gallery', '/gallery', '/en/galleryx', '/developer', '']) {
			expect(boardSession(pathname), pathname).toBe(false);
		}
	});

	it('the fixture frame says the hint, like a live viewfinder — it is not a refusal', () => {
		const m = resolveWalletFlowMessages('en');
		expect(
			scanNotice({ status: 'fixture', nothingFound: false, unusable: false }, m)
		).toBeUndefined();
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
