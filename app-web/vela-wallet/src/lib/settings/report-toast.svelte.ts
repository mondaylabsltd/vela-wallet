/**
 * The report's outcome, said where the person is when nobody is looking at
 * the report sheet any more (078, founder 2026-09-27: "反馈成功或失败都要有
 * 提示吧，而不是生硬的退出到设置页面吧").
 *
 * The sheet itself shows the outcome whenever it is open when the answer
 * arrives. This is for the other case: the sheet was closed while the report
 * was sending — the send carries on, and its ending surfaces here as a toast
 * over whatever page the person is on (the host is mounted in the root
 * layout, so leaving Settings meanwhile does not swallow it).
 *
 * Deliberately tiny and free of the sender: the root layout imports this on
 * every page, so it carries no network code and no corpus — the words are
 * handed over with the outcome, resolved by the settings route that sent it.
 */
import type { FeedbackResult } from './model';

/** The words a toast may need, from the settings corpus of the page that sent. */
export interface ReportToastCopy {
	/** `componentsUi.bugReport.successTitle` */
	filed: string;
	/** `componentsUi.bugReport.viewIssue` */
	view: string;
	/** `componentsUi.bugReport.fallbackTitle` */
	fellBack: string;
	/** `componentsUi.bugReport.openGithub` */
	open: string;
	/** `common.close` — the toast's ✕. */
	close: string;
}

export interface ReportToastModel {
	/** Changes with every toast, so a second outcome restarts the timer and the entry. */
	id: number;
	tone: 'success' | 'warning';
	title: string;
	/** Filed: the issue. Fell back: the prefilled form — the report's only road. */
	action?: { label: string; href: string };
	closeLabel: string;
	/**
	 * A filed report's toast leaves on its own after a while; a fallback's
	 * never does — its button is the one way the report still gets filed.
	 */
	persistent: boolean;
}

let nextId = 1;

/** The toast for an outcome — pure, so the two endings are tested without a page. */
export function toastFor(outcome: FeedbackResult, copy: ReportToastCopy): ReportToastModel {
	if (outcome.filed) {
		return {
			id: nextId++,
			tone: 'success',
			title: copy.filed,
			action: outcome.url === undefined ? undefined : { label: copy.view, href: outcome.url },
			closeLabel: copy.close,
			persistent: false
		};
	}
	return {
		id: nextId++,
		tone: 'warning',
		title: copy.fellBack,
		action:
			outcome.fallbackUrl === undefined
				? undefined
				: { label: copy.open, href: outcome.fallbackUrl },
		closeLabel: copy.close,
		persistent: true
	};
}

class ReportToastState {
	current = $state<ReportToastModel | null>(null);
	/**
	 * The column the toast belongs to on this page, when the page says so
	 * ({@link toastAnchor}): the toast centres on it, and while it shows the
	 * column's scroll area gains the toast's height at its end (as
	 * `--toast-room`), so the last card can scroll clear of it.
	 */
	anchor = $state<HTMLElement | null>(null);

	show(outcome: FeedbackResult, copy: ReportToastCopy): void {
		this.current = toastFor(outcome, copy);
	}

	dismiss(): void {
		this.current = null;
	}
}

/** App-resident: one toast at a time, whichever page is up. */
export const reportToast = new ReportToastState();

/**
 * `{@attach toastAnchor}` on the scroll area of the column a toast should sit
 * over — the desktop settings pane, the phone settings list. Without one, the
 * toast centres on the window.
 */
export function toastAnchor(node: HTMLElement): () => void {
	reportToast.anchor = node;
	return () => {
		if (reportToast.anchor === node) reportToast.anchor = null;
	};
}
