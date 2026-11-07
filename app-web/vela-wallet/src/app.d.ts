// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	/** Filled in by `vite.config.ts` — see `$lib/build/info` (spec 064). */
	const __VELA_VERSION__: string;
	const __VELA_COMMIT__: string;
	/** `true` in the browser extension's build (`VELA_TARGET=extension`). */
	const __VELA_EXTENSION__: boolean;
	/**
	 * `VELA_ALLOW_PRODUCTION_WRITES` as the dev server or the package build
	 * was started with (§28, `$lib/services/production-guard.ts`); `''` unset.
	 */
	const __VELA_ALLOW_PRODUCTION_WRITES__: string;
	/**
	 * The scheme the extension's own pages load from: `'chrome-extension:'`,
	 * or `'safari-web-extension:'` in the Safari package. Every other build
	 * (the hosted site, the tests) reads `'chrome-extension:'`.
	 */
	const __VELA_EXTENSION_PROTOCOL__: 'chrome-extension:' | 'safari-web-extension:';

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
		/** The history entries the app's own Back stacks write (`$lib/ui/history`). */
		interface PageState {
			velaFlow?: number;
			velaContacts?: string;
			velaCreate?: string;
		}
	}
}

export {};
