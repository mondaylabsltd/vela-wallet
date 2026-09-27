/**
 * `Deps` for `downloads.ts`, made from a real Workers request (spec 065).
 *
 * Every piece is optional on purpose. `vite dev` has no edge cache and no
 * bucket; a deploy before the founder has created the bucket has no `DOWNLOADS`
 * binding. In each case the routes still answer — by redirecting to GitHub,
 * which is rule 4 rather than a degraded mode someone has to remember.
 */
import type { RequestEvent } from '@sveltejs/kit';
import type { Deps, Mirror, ReleaseInfo, ReleaseStore } from './downloads';

/**
 * A key for the edge cache. Never requested: `caches` wants a URL, not a truth.
 *
 * The `v2` is a lever, not decoration: a deploy cannot purge what is already in
 * the cache, and v0.9.5's poisoned entry (read in the eleven minutes between
 * its Release and its macOS images) would otherwise outlive this fix by a week.
 * Bump it whenever a stored list must be abandoned.
 */
const LIST_KEY = 'https://getvela.app/__cache/downloads/latest-release.v2.json';
/** Kept long and judged by `fetchedAt`, so a stale list exists when GitHub does not. */
const LIST_KEPT_SECONDS = 7 * 24 * 3600;

function edgeStore(cache: Cache): ReleaseStore {
	return {
		async read() {
			const hit = await cache.match(LIST_KEY);
			return hit ? ((await hit.json()) as ReleaseInfo) : null;
		},
		async write(info) {
			await cache.put(
				LIST_KEY,
				new Response(JSON.stringify(info), {
					headers: {
						'Content-Type': 'application/json',
						'Cache-Control': `public, max-age=${LIST_KEPT_SECONDS}`
					}
				})
			);
		}
	};
}

export function depsFor(event: RequestEvent): Deps {
	const platform = event.platform;
	const edge = (globalThis as { caches?: { default?: Cache } }).caches?.default;
	const FixedLength = (globalThis as { FixedLengthStream?: new (n: number) => TransformStream })
		.FixedLengthStream;

	return {
		// SvelteKit's `event.fetch` would send cookies and rewrite same-origin
		// calls; these are all to GitHub, so the platform's own fetch is right.
		fetch: (input, init) => fetch(input, init),
		now: () => Date.now(),
		store: edge ? edgeStore(edge) : undefined,
		mirror: platform?.env?.DOWNLOADS as Mirror | undefined,
		waitUntil: platform?.ctx ? (work) => platform.ctx.waitUntil(work) : undefined,
		fixedLength: FixedLength
			? (body, size) => {
					const pipe = new FixedLength(size);
					// Not awaited: R2 reads the other end. A failure here fails the
					// put, which is already caught where it was started.
					body.pipeTo(pipe.writable).catch(() => {});
					return pipe.readable;
				}
			: undefined
	};
}
