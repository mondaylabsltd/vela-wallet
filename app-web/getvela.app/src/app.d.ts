// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
		interface Platform {
			env?: {
				/** R2 mirror of the latest Releases (spec 065, A5). Absent until the bucket exists. */
				DOWNLOADS?: import('@cloudflare/workers-types').R2Bucket;
			};
			ctx: { waitUntil(promise: Promise<unknown>): void };
		}
	}
}

export {};
