/**
 * A read that never answers never holds a screen for good (round 5): the
 * preview's reads (`execute.ts`) and the one review's (`proposal-review`)
 * each stop waiting after a bound and take what "said nothing" reads as.
 */

/** How long one read of a preview is waited for: past it, it said nothing. */
export const PREVIEW_READ_MS = 8_000;

/**
 * `read`, or `fallback` once it has not answered in {@link PREVIEW_READ_MS}
 * (round 5): a read that never answers never holds the preview — or the
 * buttons it decides — for good.
 */
export function bounded<T>(read: Promise<T>, fallback: T, ms = PREVIEW_READ_MS): Promise<T> {
	let timer: ReturnType<typeof setTimeout> | undefined;
	return Promise.race([
		read,
		new Promise<T>((done) => (timer = setTimeout(() => done(fallback), ms)))
	]).finally(() => clearTimeout(timer));
}
