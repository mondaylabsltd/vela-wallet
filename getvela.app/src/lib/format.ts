/** Format an ISO date (YYYY-MM-DD) as e.g. "June 12, 2026" in UTC. */
export function formatDate(iso: string): string {
	const date = new Date(`${iso}T00:00:00Z`);
	return date.toLocaleDateString('en-US', {
		year: 'numeric',
		month: 'long',
		day: 'numeric',
		timeZone: 'UTC'
	});
}

/** Escape a string for safe inclusion in XML text/attributes. */
export function escapeXml(value: string): string {
	return value
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/"/g, '&quot;')
		.replace(/'/g, '&apos;');
}
