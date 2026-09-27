/**
 * The report's send, app-resident (078, founder 2026-09-27: "反馈成功或失败都要
 * 有提示吧，而不是生硬的退出到设置页面吧").
 *
 * After Send, the sheet (the desktop's panel) never closes itself: it shows
 * the thank-you with the issue, or the fallback with the prefilled GitHub
 * form. But a person can close it while the report is still sending — the ✕,
 * the scrim, a drag down, Escape, another page in the desktop nav, another
 * tab. Then:
 *
 * - the send carries on (it lives here, not in the sheet);
 * - its ending is a toast (`report-toast.svelte.ts`) wherever the person is:
 *   filed → `successTitle` with "View on GitHub"; fell back →
 *   `fallbackTitle` with "Open GitHub form", which stays until it is used or
 *   closed, because it is the only road the report has left;
 * - a filed report is finished, so the sheet opens fresh next time; a report
 *   that fell back keeps its draft and its outcome, so the sheet reopened
 *   shows the words, the fallback block and "Try again".
 *
 * Which of the two gets the outcome is decided when it ARRIVES, by whether the
 * sheet is on screen (`surface`). The sheet's exit takes a moment to play, so
 * an outcome that lands during that moment — shown to a sheet already on its
 * way out — is said again by a toast.
 */
import type { FeedbackResult } from './model';
import { ReportDraft } from './report-draft.svelte';
import { reportToast, type ReportToastCopy } from './report-toast.svelte';

/** An outcome shown to the sheet less than this long before it closed is toasted too. */
export const LATE_CLOSE_MS = 600;

export class ReportSend {
	/** What the person has typed and attached — outlives the sheet. */
	readonly draft = new ReportDraft();
	sending = $state(false);
	/**
	 * The outcome the sheet shows: filed, or fell back. `undefined` before the
	 * first send, after Done, and after a filed report was said by a toast.
	 */
	result = $state<FeedbackResult | undefined>(undefined);

	#open = false;
	#shownAt = Number.NEGATIVE_INFINITY;
	#copy: ReportToastCopy | null = null;
	readonly #now: () => number;

	constructor(now: () => number = () => Date.now()) {
		this.#now = now;
	}

	/** The report sheet (or the desktop's report panel) came on screen, or went. */
	surface(open: boolean): void {
		const was = this.#open;
		this.#open = open;
		if (open) {
			// The sheet says it now; a toast saying it too would be twice.
			reportToast.dismiss();
			return;
		}
		const result = this.result;
		const copy = this.#copy;
		if (!was || result === undefined || copy === null) return;
		if (this.#now() - this.#shownAt < LATE_CLOSE_MS) this.#toast(result, copy);
	}

	/**
	 * Send, and say how it ended — to the sheet if it is open, else by a toast.
	 * `run` builds the payload and asks the endpoint; it resolves to an outcome
	 * for every answer, a refusal and a dead network included.
	 */
	async send(run: () => Promise<FeedbackResult>, copy: ReportToastCopy): Promise<void> {
		if (this.sending) return;
		this.sending = true;
		this.result = undefined;
		reportToast.dismiss();
		let outcome: FeedbackResult;
		try {
			outcome = await run();
		} finally {
			this.sending = false;
		}
		this.#copy = copy;
		// Filed: the words and images are on the tracker now.
		if (outcome.filed) this.draft.reset();
		if (this.#open) {
			this.result = outcome;
			this.#shownAt = this.#now();
			return;
		}
		this.#toast(outcome, copy);
	}

	#toast(outcome: FeedbackResult, copy: ReportToastCopy): void {
		reportToast.show(outcome, copy);
		// Said. A filed report is finished; a fallback stays for the sheet.
		this.result = outcome.filed ? undefined : outcome;
	}

	/** Done on the thank-you: the next report starts fresh. */
	done(): void {
		this.result = undefined;
		this.draft.reset();
	}
}

/** The one send the settings route drives — it outlives the route, too. */
export const reportSend = new ReportSend();
