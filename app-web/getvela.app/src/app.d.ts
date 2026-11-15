// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	/** Notes a reader can see in this build; set in vite.config.ts (spec 101). */
	const __NOTES_PUBLISHED__: number;

	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
	}
}

export {};
