/**
 * The part of the Workers runtime module (`cloudflare:workers`) this site uses.
 * Since adapter-cloudflare 8 the bindings and `waitUntil` come from here, not
 * from `event.platform`; under `vite dev` the adapter backs it with Wrangler's
 * platform proxy.
 *
 * Declared by hand, not by `wrangler types`: the runtime's own declarations
 * replace DOM globals (`Response`, `WebAssembly`, …) for the whole program,
 * and this program is also the browser's. The types come from the module form
 * of `@cloudflare/workers-types`, which declares no globals.
 */
declare module 'cloudflare:workers' {
	type Workers = typeof import('@cloudflare/workers-types').CloudflareWorkersModule;

	/** The bindings in wrangler.jsonc. */
	export const env: {
		/** R2 mirror of the latest Releases (spec 065, A5), also bug-report screenshots. Absent until the bucket exists. */
		DOWNLOADS?: import('@cloudflare/workers-types').R2Bucket;
	};
	export const waitUntil: Workers['waitUntil'];
}
