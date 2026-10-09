/**
 * Send a report and say how it ended, in the shape the report sheet draws
 * (spec 081 FR-016; 078). Both endings are outcomes: filed — the issue it
 * became — or the prefilled GitHub form, which is the road that still works
 * when the endpoint is unprovisioned, rate-limited or unreachable.
 *
 * One function for every door into the report: Settings → Send feedback, and
 * a relay stop's "Report this" (issue 466).
 */
import {
	BUG_REPORT_ENDPOINT,
	sendBugReport,
	type BugReportPayload
} from '$lib/services/bug-report';
import type { FeedbackResult } from './model';

export async function fileReport(
	payload: BugReportPayload,
	endpoint: string = BUG_REPORT_ENDPOINT
): Promise<FeedbackResult> {
	const outcome = await sendBugReport(payload, endpoint);
	return outcome.ok
		? {
				filed: true,
				number: outcome.number,
				url: outcome.url,
				deduped: outcome.deduped,
				screenshotsDropped: outcome.screenshotsDropped
			}
		: {
				filed: false,
				fallbackUrl: outcome.fallbackUrl,
				withScreenshots: (payload.screenshots?.length ?? 0) > 0
			};
}
