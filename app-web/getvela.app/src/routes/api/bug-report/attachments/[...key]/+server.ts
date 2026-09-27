/**
 * A bug report's screenshot, as the issue shows it.
 *
 * GitHub cannot host an image for an issue filed through its API, so the
 * report route stores each screenshot in R2 and the issue links here. Only
 * keys the report route minted are served — never anything else in the
 * bucket — and each as the image type its bytes were sniffed to be. See
 * `$lib/server/bug-report`.
 */
import type { RequestHandler } from './$types';
import { serveAttachment, type AttachmentBucket } from '$lib/server/bug-report';

export const GET: RequestHandler = ({ params, platform }) =>
	serveAttachment(params.key, platform?.env?.DOWNLOADS as AttachmentBucket | undefined);
