import type { RequestHandler } from './$types';

// Google Search Console ownership verification.
//
// Served as a DYNAMIC route (not a file in static/) on purpose: Cloudflare's
// asset handler 307-redirects any "*.html" path to its extension-less form,
// which breaks Google's check that fetches this exact URL and expects a 200.
// A worker endpoint returns 200 directly with no redirect. Same pattern the
// site already uses for sitemap.xml.
export const prerender = false;

export const GET: RequestHandler = () =>
	new Response('google-site-verification: google517977f8d4c51496.html\n', {
		headers: { 'Content-Type': 'text/html; charset=utf-8' }
	});
