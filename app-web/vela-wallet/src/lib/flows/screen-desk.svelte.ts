/**
 * The pages' own screens, reachable from outside them (spec 103).
 *
 * Which asset, transaction, contact or settings page is open is each page's
 * own state — the wallet page's `selectedAssetId`, the contacts page's `ui`,
 * Settings' `page` — so a `show` intent has no other way to it. While mounted,
 * a page attaches a presenter for the screens it draws: the same code its
 * own rows run when tapped. Nothing about what is drawn is decided here.
 */
import type { IntentScreen } from '$lib/core/generated/IntentScreen';

export type ScreenKind = IntentScreen['type'];

/** Open one screen on the mounted page: why it could not, or `null`. */
export type Presenter = (screen: IntentScreen) => string | null | Promise<string | null>;

/** How long a page may take to mount and attach its presenter. */
const MOUNT_MS = 8_000;

class ScreenDesk {
	#presenters = new Map<ScreenKind, Presenter>();

	/** Attach a page's presenter; returns the detach for its unmount. */
	attach(kinds: ScreenKind[], presenter: Presenter): () => void {
		for (const kind of kinds) this.#presenters.set(kind, presenter);
		return () => {
			for (const kind of kinds)
				if (this.#presenters.get(kind) === presenter) this.#presenters.delete(kind);
		};
	}

	/** Open `screen` once the page that draws it is mounted. */
	async present(screen: IntentScreen): Promise<string | null> {
		const deadline = Date.now() + MOUNT_MS;
		for (;;) {
			const presenter = this.#presenters.get(screen.type);
			if (presenter) return presenter(screen);
			if (Date.now() >= deadline) return 'the page that draws it did not open';
			await new Promise((done) => setTimeout(done, 50));
		}
	}
}

export const screenDesk = new ScreenDesk();
