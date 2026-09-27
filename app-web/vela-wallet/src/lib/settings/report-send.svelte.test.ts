/**
 * The outcome is always shown (078, founder 2026-09-27: "反馈成功或失败都要有
 * 提示吧，而不是生硬的退出到设置页面吧"): in the sheet when it is open when the
 * answer arrives, else by a toast — filed with "View on GitHub", fell back
 * with "Open GitHub form". Nothing here reaches the network: `run` stands in
 * for the endpoint.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { FeedbackResult } from './model';
import { LATE_CLOSE_MS, ReportSend } from './report-send.svelte';
import { reportToast, toastAnchor, toastFor, type ReportToastCopy } from './report-toast.svelte';
import ReportToastHost from './ui/ReportToastHost.svelte';

const COPY: ReportToastCopy = {
	filed: 'Thanks — your report is in',
	view: 'View on GitHub',
	fellBack: "Couldn't send from the app",
	open: 'Open GitHub form',
	close: 'Close'
};
const FILED: FeedbackResult = {
	filed: true,
	number: 12,
	url: 'https://github.com/x/y/issues/12',
	deduped: false,
	screenshotsDropped: 0
};
const FELL: FeedbackResult = {
	filed: false,
	fallbackUrl: 'https://github.com/x/y/issues/new?title=%5BWeb%5D+x',
	withScreenshots: false
};

/** An endpoint that answers when told to. */
function endpoint(outcome: FeedbackResult) {
	let answer!: () => void;
	const gate = new Promise<void>((resolve) => (answer = resolve));
	return { run: async () => (await gate, outcome), answer };
}

afterEach(() => {
	reportToast.dismiss();
	vi.useRealTimers();
});

describe('where the outcome is said', () => {
	it('in the sheet when it is open when the answer arrives — and no toast', async () => {
		const send = new ReportSend();
		send.surface(true);
		const done = send.send(async () => FILED, COPY);
		expect(send.sending).toBe(true);
		await done;
		expect(send.sending).toBe(false);
		expect(send.result).toEqual(FILED);
		expect(reportToast.current).toBeNull();
	});

	it('closed mid-send and filed: a toast with "View on GitHub", and the next report starts fresh', async () => {
		const send = new ReportSend();
		send.draft.what = 'Balance shows 0';
		send.surface(true);
		const { run, answer } = endpoint(FILED);
		const done = send.send(run, COPY);
		send.surface(false);
		answer();
		await done;
		expect(reportToast.current).toMatchObject({
			tone: 'success',
			title: COPY.filed,
			action: { label: COPY.view, href: FILED.url },
			persistent: false
		});
		// Said by the toast; the sheet opens fresh.
		expect(send.result).toBeUndefined();
		expect(send.draft.what).toBe('');
	});

	it('closed mid-send and fell back: a toast with "Open GitHub form" that stays, and the words wait in the sheet', async () => {
		const send = new ReportSend();
		send.draft.what = 'Balance shows 0';
		send.surface(true);
		const { run, answer } = endpoint(FELL);
		const done = send.send(run, COPY);
		send.surface(false);
		answer();
		await done;
		expect(reportToast.current).toMatchObject({
			tone: 'warning',
			title: COPY.fellBack,
			action: { label: COPY.open, href: FELL.fallbackUrl },
			persistent: true
		});
		expect(send.result).toEqual(FELL);
		expect(send.draft.what).toBe('Balance shows 0');
		// Reopened, the sheet says it — the toast would be saying it twice.
		send.surface(true);
		expect(reportToast.current).toBeNull();
	});

	it('an answer that lands as the sheet is on its way out is toasted too', () => {
		let now = 1000;
		const send = new ReportSend(() => now);
		send.surface(true);
		return send
			.send(async () => FELL, COPY)
			.then(() => {
				expect(reportToast.current).toBeNull();
				now += LATE_CLOSE_MS - 100;
				send.surface(false);
				expect(reportToast.current?.title).toBe(COPY.fellBack);
			});
	});

	it('but a sheet closed long after showing its outcome raises no toast', async () => {
		let now = 1000;
		const send = new ReportSend(() => now);
		send.surface(true);
		await send.send(async () => FILED, COPY);
		now += LATE_CLOSE_MS + 5000;
		send.surface(false);
		expect(reportToast.current).toBeNull();
	});

	it('Done starts over; a second Send while one is in flight does nothing', async () => {
		const send = new ReportSend();
		send.surface(true);
		const { run, answer } = endpoint(FILED);
		const first = send.send(run, COPY);
		const second = vi.fn(async () => FELL);
		await send.send(second, COPY);
		expect(second).not.toHaveBeenCalled();
		answer();
		await first;
		send.draft.what = 'x';
		send.done();
		expect(send.result).toBeUndefined();
		expect(send.draft.what).toBe('');
	});

	it('a new send takes a toast still showing away', async () => {
		const send = new ReportSend();
		await send.send(async () => FELL, COPY);
		expect(reportToast.current).not.toBeNull();
		const { run, answer } = endpoint(FILED);
		const done = send.send(run, COPY);
		expect(reportToast.current).toBeNull();
		answer();
		await done;
	});
});

describe('the toast', () => {
	it('filed: the title, a link to the issue in a new tab, and ✕', async () => {
		const screen = render(ReportToastHost);
		reportToast.show(FILED, COPY);
		await tick();
		const toast = screen.container.querySelector('.toast') as HTMLElement;
		expect(toast.textContent).toContain(COPY.filed);
		const link = toast.querySelector('a') as HTMLAnchorElement;
		expect(link.textContent?.trim()).toBe(COPY.view);
		expect(link.getAttribute('href')).toBe(FILED.url);
		expect(link.getAttribute('target')).toBe('_blank');
		expect(link.getAttribute('rel')).toBe('noopener noreferrer');
		// Said politely, from a region that was already there.
		expect(screen.container.querySelector('[aria-live="polite"]')?.contains(toast)).toBe(true);
		(toast.querySelector('button.close') as HTMLButtonElement).click();
		await tick();
		expect(screen.container.querySelector('.toast')).toBeNull();
	});

	it('fell back: the title and the prefilled form', async () => {
		const screen = render(ReportToastHost);
		reportToast.show(FELL, COPY);
		await tick();
		const toast = screen.container.querySelector('.toast') as HTMLElement;
		expect(toast.textContent).toContain(COPY.fellBack);
		expect(toast.querySelector('a')?.getAttribute('href')).toBe(FELL.fallbackUrl);
		expect(toast.querySelector('a')?.textContent?.trim()).toBe(COPY.open);
	});

	it('a filed toast leaves on its own; a fallback’s stays until it is used or closed', async () => {
		vi.useFakeTimers();
		const screen = render(ReportToastHost);
		reportToast.show(FILED, COPY);
		await tick();
		expect(screen.container.querySelector('.toast')).not.toBeNull();
		await vi.advanceTimersByTimeAsync(9000);
		expect(reportToast.current).toBeNull();
		reportToast.show(FELL, COPY);
		await tick();
		await vi.advanceTimersByTimeAsync(60_000);
		expect(reportToast.current?.title).toBe(COPY.fellBack);
		// Using it puts it away. (The navigation itself is cancelled: a test
		// opens no GitHub tab.)
		const link = screen.container.querySelector('.toast a') as HTMLAnchorElement;
		link.addEventListener('click', (event) => event.preventDefault());
		link.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
		await tick();
		expect(reportToast.current).toBeNull();
	});

	it('is built from the outcome alone', () => {
		expect(toastFor({ filed: true, number: 3 }, COPY).action).toBeUndefined();
		expect(toastFor(FELL, COPY).persistent).toBe(true);
	});
});

describe('where the toast sits', () => {
	it('centres on the page’s own column, and that column makes room for it at its end', async () => {
		const column = document.createElement('div');
		column.style.cssText = 'position: fixed; left: 300px; top: 0; width: 800px; height: 600px';
		document.body.appendChild(column);
		const release = toastAnchor(column);
		try {
			const screen = render(ReportToastHost);
			reportToast.show(FELL, COPY);
			await tick();
			const region = screen.container.querySelector('.region') as HTMLElement;
			await vi.waitFor(() => {
				expect(region.getBoundingClientRect().left).toBeCloseTo(300, 0);
				expect(region.getBoundingClientRect().width).toBeCloseTo(800, 0);
			});
			const toast = region.querySelector('.toast') as HTMLElement;
			const middle = toast.getBoundingClientRect().left + toast.getBoundingClientRect().width / 2;
			expect(middle).toBeCloseTo(300 + 400, 0);
			// Room at the column's end, the toast's height and a gap.
			await vi.waitFor(() => expect(column.style.getPropertyValue('--toast-room')).toContain('px'));
			reportToast.dismiss();
			await tick();
			await vi.waitFor(() => expect(column.style.getPropertyValue('--toast-room')).toBe(''));
		} finally {
			release();
			column.remove();
		}
		expect(reportToast.anchor).toBeNull();
	});

	it('with no column named, centres on the window', async () => {
		const screen = render(ReportToastHost);
		reportToast.show(FILED, COPY);
		await tick();
		const region = screen.container.querySelector('.region') as HTMLElement;
		expect(region.classList.contains('anchored')).toBe(false);
		expect(Math.round(region.getBoundingClientRect().left)).toBe(0);
	});
});
