/**
 * What the person has put into the report so far — the words, the steps and
 * the screenshots — held OUTSIDE the sheet that edits it (078, founder
 * 2026-09-27: "反馈成功或失败都要有提示吧，而不是生硬的退出到设置页面吧").
 *
 * The sheet can close at any moment: the ✕, a tap on the scrim, a drag down
 * (every phone sheet drags closed since 08574f88), Escape, or a different
 * page in the desktop's nav. Before this, closing it unmounted the words with
 * it — and a send still in flight, or one that had fallen back to the GitHub
 * form, then pointed at a report nobody could see again. Kept here, the draft
 * survives the sheet: reopened, it shows what was typed and attached, and a
 * send that was waiting for a tile to be prepared goes on waiting and sends.
 *
 * The sheet makes its own draft when it is handed none (the gallery, the
 * component tests) and discards it when it goes; the settings route hands it
 * the app-resident one (`report-send.svelte.ts`).
 *
 * Each image is decoded and re-encoded as a JPEG on this device before it is
 * a tile (`screenshot-prep.ts`) — which is what strips its EXIF and location
 * — and the tile shows those processed bytes, the ones sent.
 */
import {
	chooseScreenshots,
	prepareScreenshot,
	type ScreenshotRefusal
} from '$lib/services/screenshot-prep';

/** One tile: still being prepared, or ready with the bytes that will be sent. */
export interface Shot {
	id: number;
	/** The processed JPEG, for the tile and the viewer. Absent while preparing. */
	url?: string;
	/** The same JPEG as plain base64 — what Send hands over. */
	base64?: string;
}

export class ReportDraft {
	what = $state('');
	steps = $state('');
	stepsOpen = $state(false);
	shots = $state<Shot[]>([]);
	/** A refusal (a sixth image, a file that is not an image), until the next change. */
	refusal = $state<ScreenshotRefusal | null>(null);
	/**
	 * Send was pressed while a tile was still being prepared: the person
	 * attached it and expects it to go, so the send waits for it — busy, with
	 * the sending words — instead of leaving it behind. Here rather than in the
	 * sheet, so a sheet reopened during that wait still says so.
	 */
	waiting = $state(false);

	/**
	 * Tiles still being prepared, by id: what a send waits for. Bookkeeping,
	 * never drawn — the tiles themselves are the reactive state.
	 */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- deliberately not reactive (see above)
	readonly #preparing = new Map<number, Promise<void>>();
	#nextId = 0;

	/** Take files from any of the three doors: the picker, a paste, a drop. */
	add(files: readonly File[], max: number): void {
		if (files.length === 0) return;
		const chosen = chooseScreenshots(this.shots.length, files, max);
		this.refusal = chosen.refusal;
		for (const file of chosen.take) {
			const id = this.#nextId++;
			this.shots.push({ id });
			const job = prepareScreenshot(file).then(
				(prepared) => {
					const at = this.shots.findIndex((s) => s.id === id);
					if (at === -1) return; // removed while it was being prepared
					this.shots[at] = {
						...this.shots[at],
						url: URL.createObjectURL(prepared.blob),
						base64: prepared.base64
					};
				},
				() => {
					// The browser could not decode it (HEIC in most of them):
					// refused, never sent as it was.
					this.remove(id, false);
					this.refusal = 'unsupported';
				}
			);
			this.#preparing.set(id, job);
			void job.finally(() => this.#preparing.delete(id));
		}
	}

	remove(id: number, clearNotice = true): void {
		const at = this.shots.findIndex((s) => s.id === id);
		if (at === -1) return;
		const [gone] = this.shots.splice(at, 1);
		if (gone.url) URL.revokeObjectURL(gone.url);
		if (clearNotice) this.refusal = null;
	}

	/** Some tile is still being prepared. */
	get preparing(): boolean {
		return this.#preparing.size > 0;
	}

	/** Every tile has been prepared — or refused. */
	async settled(): Promise<void> {
		while (this.#preparing.size > 0) {
			await Promise.allSettled([...this.#preparing.values()]);
		}
	}

	/** The processed JPEGs, as base64, in tile order — what Send carries. */
	screenshots(): string[] {
		return this.shots.flatMap((s) => (s.base64 === undefined ? [] : [s.base64]));
	}

	/** Start over: the report went, or the gallery's picture is being taken down. */
	reset(): void {
		for (const shot of this.shots) if (shot.url) URL.revokeObjectURL(shot.url);
		this.shots = [];
		this.what = '';
		this.steps = '';
		this.stepsOpen = false;
		this.refusal = null;
		this.waiting = false;
	}
}
