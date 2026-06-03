import { dev } from '$app/environment';
import type { RequestHandler } from './$types';

const BLOCKED_HOSTS = ['localhost', '127.0.0.1', '[::1]', '0.0.0.0'];
const BLOCKED_TLDS = ['.local', '.internal', '.localhost'];

function isBlockedUrl(urlStr: string): boolean {
	try {
		const url = new URL(urlStr);
		const hostname = url.hostname;

		// Block known local hosts
		if (BLOCKED_HOSTS.includes(hostname)) return true;

		// Block local TLDs
		if (BLOCKED_TLDS.some((tld) => hostname.endsWith(tld))) return true;

		// Block private IP ranges
		if (/^10\./.test(hostname)) return true;
		if (/^192\.168\./.test(hostname)) return true;
		if (/^172\.(1[6-9]|2\d|3[01])\./.test(hostname)) return true;

		// Require HTTPS in production
		if (!dev && url.protocol !== 'https:') return true;

		return false;
	} catch {
		return true;
	}
}

export const GET: RequestHandler = async ({ url }) => {
	const targetUrl = url.searchParams.get('url');

	if (!targetUrl) {
		return new Response(JSON.stringify({ error: 'Missing url parameter' }), {
			status: 400,
			headers: { 'Content-Type': 'application/json' }
		});
	}

	if (isBlockedUrl(targetUrl)) {
		return new Response(JSON.stringify({ error: 'Blocked URL' }), {
			status: 403,
			headers: { 'Content-Type': 'application/json' }
		});
	}

	try {
		const upstream = await fetch(targetUrl);

		const contentType = upstream.headers.get('content-type') || '';

		// SSE stream — pipe through
		if (contentType.includes('text/event-stream') && upstream.body) {
			return new Response(upstream.body, {
				status: 200,
				headers: {
					'Content-Type': 'text/event-stream',
					'Cache-Control': 'no-cache, no-store, must-revalidate',
					'X-Accel-Buffering': 'no'
				}
			});
		}

		// Regular response — forward body and status
		return new Response(upstream.body, {
			status: upstream.status,
			headers: {
				'Content-Type': contentType || 'application/json'
			}
		});
	} catch {
		return new Response(JSON.stringify({ error: 'Upstream request failed' }), {
			status: 502,
			headers: { 'Content-Type': 'application/json' }
		});
	}
};
