/**
 * The dead-selector watchdog (2026-09-19).
 *
 * On Android 14+ a `credentials.get()` whose candidate list outgrows a binder
 * transaction never settles: the system's sheet fails to start and nobody is
 * told. What these pin is the rule that tells that apart from a person who is
 * simply taking their time — focus held for the whole window means no sheet is
 * up; while one is up, focus is elsewhere.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { authenticate, SELECTOR_UNRESPONSIVE } from './passkey';
import { promptCopy } from './copy';

let focused = true;

/** A `credentials.get()` that settles only when its signal aborts — the hang. */
function hangingGet(): ReturnType<typeof vi.fn> {
	return vi.fn(
		({ signal }: { signal: AbortSignal }) =>
			new Promise((_, reject) => {
				signal.addEventListener('abort', () => reject(signal.reason));
			})
	);
}

function browser(userAgent: string, get: ReturnType<typeof vi.fn>): void {
	const location = { protocol: 'https:', hostname: 'getvela.app' };
	vi.stubGlobal('window', { location, PublicKeyCredential: class {} });
	vi.stubGlobal('location', location);
	vi.stubGlobal('navigator', { userAgent, credentials: { get } });
	vi.stubGlobal('document', { hasFocus: () => focused, visibilityState: 'visible' });
}

const ANDROID = 'Mozilla/5.0 (Linux; Android 16; 24129PN74C) Chrome/140 Mobile Safari/537.36';
const MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Chrome/140 Safari/537.36';

beforeEach(() => {
	focused = true;
	vi.useFakeTimers();
});

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('dead passkey selector', () => {
	it('gives up when the page kept focus the whole time, and says why', async () => {
		browser(ANDROID, hangingGet());
		const attempt = authenticate().catch((error) => error);
		await vi.advanceTimersByTimeAsync(10_000);
		const error = await attempt;
		expect(error.kind).toBe('other');
		expect(error.message).toBe(SELECTOR_UNRESPONSIVE);
	});

	it('waits as long as the sheet holds focus, however long that is', async () => {
		browser(ANDROID, hangingGet());
		let settled = false;
		void authenticate().catch(() => (settled = true));
		await vi.advanceTimersByTimeAsync(1_000);
		focused = false; // the sheet is up
		await vi.advanceTimersByTimeAsync(120_000);
		expect(settled).toBe(false);
	});

	it('still gives up when a dead selector borrowed focus for a moment', async () => {
		browser(ANDROID, hangingGet());
		const attempt = authenticate().catch((error) => error);
		await vi.advanceTimersByTimeAsync(500);
		focused = false; // the window manager waiting on a window that never comes
		await vi.advanceTimersByTimeAsync(5_000);
		focused = true;
		await vi.advanceTimersByTimeAsync(10_000);
		expect((await attempt).message).toBe(SELECTOR_UNRESPONSIVE);
	});

	it('never runs off Android, where the dialog need not take focus', async () => {
		browser(MAC, hangingGet());
		let settled = false;
		void authenticate().catch(() => (settled = true));
		await vi.advanceTimersByTimeAsync(60_000);
		expect(settled).toBe(false);
	});

	it('is shown in the shell’s words, not as a marker', () => {
		const t = (key: string) => key;
		const copy = promptCopy({ type: 'sign_in_failed', detail: SELECTOR_UNRESPONSIVE }, t);
		expect(copy.message).toBe('onboarding.login.alertSelectorUnresponsive');
	});
});
