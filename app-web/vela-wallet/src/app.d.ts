// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	/** Filled in by `vite.config.ts` — see `$lib/build/info` (spec 064). */
	const __VELA_VERSION__: string;
	const __VELA_COMMIT__: string;
	/** `true` in the browser extension's build (`VELA_TARGET=extension`). */
	const __VELA_EXTENSION__: boolean;

	namespace App {
		interface Platform {
			env: Env;
			ctx: ExecutionContext;
			caches: CacheStorage;
			cf?: IncomingRequestCfProperties;
		}

		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
	}
}

export {};
