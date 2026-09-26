<script lang="ts">
	/**
	 * ST15 — the in-app report. The disclosure showing exactly what will be
	 * sent is open by default and the consent note sits directly above the send
	 * button, because the promise is only worth anything next to the thing it is
	 * a promise about.
	 *
	 * Spec 081 FR-016 gave the button somewhere to go; 078 round 3 (founder,
	 * 2026-09-26: "方便把截图上传，多张截图也要能上传") adds screenshots:
	 *
	 * - **Up to five**, picked (the hidden input behind both add targets),
	 *   pasted anywhere while the sheet is open, or dropped on the section.
	 *   Each is decoded and re-encoded as a JPEG on this device before it is a
	 *   tile (`screenshot-prep.ts`) — which is what strips its EXIF and
	 *   location — and the tile shows those processed bytes, the ones sent.
	 * - **Public**: the founder ruled screenshots are shown in the GitHub
	 *   issue, so the line saying so is on screen under the tiles, before
	 *   Send, never folded into a disclosure.
	 * - **Never in the way**: a refusal (a sixth image, a file that is not an
	 *   image) is a line of its own ABOVE the public line until the next change
	 *   — never in its place (v2 A1) — and nothing about screenshots ever
	 *   disables Send. A tile still being prepared when Send is pressed is
	 *   waited for, not dropped.
	 *
	 * The two outcomes are drawn here rather than in a second sheet: a report
	 * that filed says which issue it became (and, if its images could not be
	 * stored, says that too); a report the endpoint refused offers the
	 * prefilled form, which is the ONLY remaining road and must never be
	 * replaced by an apology — and says the images cannot follow it there.
	 *
	 * Icons beside words are sized in em (v2 A8), so they grow with the text
	 * scale instead of staying the size the standard text was drawn at.
	 */
	import { onDestroy, untrack } from 'svelte';
	import { fade, scale } from 'svelte/transition';
	import type { FeedbackModel, FeedbackResult } from '../model';
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { fill } from '$lib/wallet/messages';
	import {
		chooseScreenshots,
		imageFilesOf,
		prepareScreenshot,
		SCREENSHOT_ACCEPT,
		type ScreenshotRefusal
	} from '$lib/services/screenshot-prep';
	import Disclosure from './Disclosure.svelte';

	interface Props {
		panel: FeedbackModel;
		/**
		 * Absent in the gallery, where this sheet is a picture of itself.
		 * `screenshots` are the processed JPEGs' base64, in tile order.
		 */
		onsend?: (report: { what: string; steps: string; screenshots: string[] }) => void;
		/** The route is waiting on the endpoint. */
		sending?: boolean;
		/** The last send's outcome, or `undefined` before the first. */
		result?: FeedbackResult;
		/** 完成 on the filed state: the host closes the sheet (or resets the panel). */
		ondone?: () => void;
	}

	let { panel, onsend, sending = false, result, ondone }: Props = $props();

	let what = $state('');
	let steps = $state('');
	let stepsOpen = $state(false);

	/** One tile: still being prepared, or ready with the bytes that will be sent. */
	interface Shot {
		id: number;
		url?: string;
		base64?: string;
	}
	let shots = $state<Shot[]>([]);
	let refusal = $state<ScreenshotRefusal | null>(null);
	let dropping = $state(false);
	/**
	 * Send was pressed while a tile was still being prepared: the person
	 * attached it and expects it to go, so the send waits for it — busy, with
	 * the sending words — instead of leaving it behind.
	 */
	let waiting = $state(false);
	/**
	 * Tiles still being prepared, by id: what a send waits for. Bookkeeping,
	 * never drawn — the tiles themselves are the reactive state.
	 */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- deliberately not reactive (see above)
	const preparing = new Map<number, Promise<void>>();
	let alive = true;
	/** Whether the send in flight carried images — the fallback says they cannot follow. */
	let sentWithShots = $state(false);
	let picker = $state<HTMLInputElement | undefined>();
	let nextId = 0;

	const max = $derived(panel.screenshots.max);
	const ready = $derived(what.trim() !== '');
	const live = $derived(onsend !== undefined);
	const fallen = $derived(
		result !== undefined && result.filed === false && result.fallbackUrl !== undefined
	);

	/**
	 * "What will be sent", as label / value rows (v2 A4). Each line is the
	 * payload's own `environment` line, split at its first ": " — the view
	 * never re-words what is sent, it only lays it out.
	 */
	const rows = $derived(
		panel.previewLines.map((line) => {
			const at = line.indexOf(': ');
			return at === -1
				? { label: undefined, value: line }
				: { label: line.slice(0, at), value: line.slice(at + 2) };
		})
	);

	/**
	 * A user agent is one long "word"; let it wrap after its slashes
	 * (`AppleWebKit/` · `537.36`) rather than mid-number. Only the drawing
	 * changes — the text, and what is sent, are the same.
	 */
	function slashParts(value: string): string[] {
		return value.split(/(?<=\/)/);
	}

	let root = $state<HTMLDivElement | undefined>();
	let filedTitle = $state<HTMLElement | undefined>();
	let fallbackTitle = $state<HTMLElement | undefined>();
	let fallbackRoad = $state<HTMLElement | undefined>();

	/** Sending, or waiting on a tile before sending: the form holds still (v3 B9). */
	const busy = $derived(sending || waiting);

	/**
	 * Each outcome is said where the person is (v3 B6/B7). Filed: focus moves
	 * to the thank-you's title, so a screen reader reads it and a keyboard is
	 * not dropped on the page body. Fell back: the block AND its "Open the
	 * GitHub form" button are scrolled into view — on a real-height phone they
	 * landed below the fold — and focus goes to the block's title.
	 *
	 * Only when the outcome ARRIVES while this sheet is open: the route keeps
	 * the last outcome, so reopening the sheet (or coming back to the panel)
	 * remounts with it — and must not grab focus or scroll again.
	 */
	const outcomeAtMount = untrack(() => result);
	/** Names the outcome blocks for assistive tech (the title is also where focus goes). */
	const uid = $props.id();
	$effect(() => {
		const outcome = result;
		if (outcome === undefined || outcome === outcomeAtMount) return;
		if (outcome.filed) {
			filedTitle?.focus({ preventScroll: true });
		} else if (fallbackRoad !== undefined) {
			fallbackTitle?.focus({ preventScroll: true });
			const still = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
			fallbackRoad.scrollIntoView({ block: 'nearest', behavior: still ? 'auto' : 'smooth' });
		}
	});
	/**
	 * The height the thank-you keeps (v2 A7): the form's at Send, but never
	 * more than the part of the sheet (or window) it is seen in. Holding it
	 * means the sheet does not collapse under the person's thumb, and gives
	 * the success block a frame to sit at the optical centre of.
	 */
	let held = $state(0);

	function visibleHeight(el: HTMLElement): number {
		let scroller = el.parentElement;
		while (scroller !== null && !/(auto|scroll)/.test(getComputedStyle(scroller).overflowY)) {
			scroller = scroller.parentElement;
		}
		const top = el.getBoundingClientRect().top;
		const room =
			scroller === null
				? window.innerHeight - (top + window.scrollY)
				: scroller.clientHeight - (top - scroller.getBoundingClientRect().top + scroller.scrollTop);
		return Math.max(0, Math.min(el.offsetHeight, room));
	}

	/** Take files from any of the three doors: the picker, a paste, a drop. */
	function add(files: readonly File[]): void {
		if (!live || busy || files.length === 0) return;
		const chosen = chooseScreenshots(shots.length, files, max);
		refusal = chosen.refusal;
		for (const file of chosen.take) {
			const shot: Shot = { id: nextId++ };
			shots.push(shot);
			const job = prepareScreenshot(file).then(
				(prepared) => {
					const at = shots.findIndex((s) => s.id === shot.id);
					if (at === -1) return; // removed while it was being prepared
					shots[at] = {
						...shots[at],
						url: URL.createObjectURL(prepared.blob),
						base64: prepared.base64
					};
				},
				() => {
					// The browser could not decode it (HEIC in most of them): refused,
					// never sent as it was.
					remove(shot.id, false);
					refusal = 'unsupported';
				}
			);
			preparing.set(shot.id, job);
			void job.finally(() => preparing.delete(shot.id));
		}
	}

	function remove(id: number, clearNotice = true): void {
		const at = shots.findIndex((s) => s.id === id);
		if (at === -1) return;
		const [gone] = shots.splice(at, 1);
		if (gone.url) URL.revokeObjectURL(gone.url);
		if (clearNotice) refusal = null;
	}

	function picked(event: Event & { currentTarget: HTMLInputElement }): void {
		const input = event.currentTarget;
		add(Array.from(input.files ?? []));
		// Picking the same file again must fire `change` again.
		input.value = '';
	}

	/** Paste anywhere in the sheet: images only — text keeps pasting as text. */
	function pasted(event: ClipboardEvent): void {
		if (result?.filed === true || busy) return;
		const files = imageFilesOf(event.clipboardData).filter((f) => f.type.startsWith('image/'));
		if (files.length === 0) return;
		event.preventDefault();
		add(files);
	}

	function dropped(event: DragEvent): void {
		event.preventDefault();
		dropping = false;
		add(imageFilesOf(event.dataTransfer));
	}

	function dragOver(event: DragEvent): void {
		if (!live || !event.dataTransfer?.types.includes('Files')) return;
		event.preventDefault();
		dropping = true;
	}

	/**
	 * Screenshots are optional — they never stop a report going — but one
	 * that was attached is never left behind: a tile still being prepared is
	 * waited for (busy, with the sending words), and one that cannot be
	 * prepared is dropped with the "couldn't be opened" line while the rest
	 * go.
	 */
	async function send(): Promise<void> {
		if (waiting) return;
		if (root !== undefined) held = visibleHeight(root);
		// A send is the next change: the refusal has been read. (A tile that
		// fails while this send waits says so again, below.)
		refusal = null;
		if (preparing.size > 0) {
			waiting = true;
			await Promise.allSettled([...preparing.values()]);
			waiting = false;
			if (!alive) return;
		}
		const screenshots = shots.flatMap((s) => (s.base64 === undefined ? [] : [s.base64]));
		sentWithShots = screenshots.length > 0;
		onsend?.({ what, steps, screenshots });
	}

	function done(): void {
		for (const shot of shots) if (shot.url) URL.revokeObjectURL(shot.url);
		shots = [];
		what = '';
		steps = '';
		stepsOpen = false;
		refusal = null;
		held = 0;
		ondone?.();
	}

	onDestroy(() => {
		alive = false;
		for (const shot of shots) if (shot.url) URL.revokeObjectURL(shot.url);
	});
</script>

<svelte:window onpaste={pasted} />

<div class="feedback" bind:this={root}>
	{#if result?.filed === true}
		{@const dropped = (result.screenshotsDropped ?? 0) > 0}
		<!-- Filed. The number is the whole point: a person who reported
		     something is owed a way back to it. Held at the form's height with
		     the block at the 2:3 optical centre (v2 A7). No accent: nothing
		     here moves value — unless images were lost, and then the issue page
		     is where they get added, so "View" is the one that leads. Done is
		     always the quiet text button under it. -->
		<div
			class="filed"
			class:held={held > 0}
			style:--held={held > 0 ? `${held}px` : undefined}
			in:fade={{ duration: 150 }}
		>
			<div class="filed-block" role="group" aria-labelledby="{uid}-filed">
				<span class="disc"><Icon icon={UTILITY_ICONS.check} size="tab" /></span>
				<p
					id="{uid}-filed"
					class="filed-title"
					tabindex="-1"
					data-focus-inner
					bind:this={filedTitle}
				>
					{panel.success.title}
				</p>
				<p class="filed-body">
					{fill(result.deduped === true ? panel.success.bodyDeduped : panel.success.bodyNew, {
						number: result.number ?? 0
					})}
				</p>
				{#if dropped}
					<p class="filed-dropped">{panel.success.dropped}</p>
				{/if}
				<div class="filed-actions">
					{#if result.url !== undefined}
						<Button
							variant={dropped ? 'primary' : 'secondary'}
							shape="rounded"
							href={result.url}
							external>{panel.success.view}</Button
						>
					{/if}
					<!-- Done is a plain text button (v3 B3): two equal outlined pills
					     had no hierarchy. -->
					<button type="button" class="done" onclick={done}>{panel.success.done}</button>
				</div>
			</div>
		</div>
	{:else}
		<textarea
			data-field
			inert={busy}
			bind:value={what}
			placeholder={panel.placeholder}
			aria-label={panel.placeholder}
			rows="4"></textarea>

		{#if stepsOpen}
			<textarea
				data-field
				inert={busy}
				bind:value={steps}
				placeholder={panel.stepsPlaceholder}
				aria-label={panel.stepsPlaceholder}
				rows="3"></textarea>
		{:else}
			<button type="button" class="steps" inert={busy} onclick={() => (stepsOpen = true)}
				>{panel.addSteps}</button
			>
		{/if}

		<!-- The screenshots. The whole section takes a drop; paste works from
		     anywhere in the sheet (the window listener above). -->
		<!-- Inert while sending (v3 B9): what is being sent does not change
		     under the spinner. -->
		<section
			class="shots"
			class:dropping
			inert={busy}
			aria-label={panel.screenshots.label}
			ondragover={dragOver}
			ondragleave={() => (dropping = false)}
			ondrop={dropped}
		>
			<div class="shots-head">
				<span class="shots-label">{panel.screenshots.label}</span>
				<span class="shots-count">
					{shots.length === 0 ? panel.screenshots.hint : `${shots.length} / ${max}`}
				</span>
			</div>

			<input
				bind:this={picker}
				class="picker"
				type="file"
				accept={SCREENSHOT_ACCEPT}
				multiple
				tabindex="-1"
				aria-hidden="true"
				onchange={picked}
			/>

			{#if shots.length === 0}
				<button type="button" class="add-target" onclick={() => picker?.click()}>
					<span class="add-line">
						<Icon icon={UTILITY_ICONS['image-plus']} />
						<span>{panel.screenshots.add}</span>
					</span>
					<span class="drop-hint">{panel.screenshots.dropHint}</span>
				</button>
			{:else}
				<!-- One row, always (v2 A6): five equal columns that shrink
				     together on a narrow screen instead of wrapping a lone tile
				     onto a second line. -->
				<ul class="tiles">
					{#each shots as shot, i (shot.id)}
						<li class="tile" transition:scale={{ duration: 150, start: 0.9 }}>
							{#if shot.url !== undefined}
								<img src={shot.url} alt="" />
							{:else}
								<span class="processing" aria-hidden="true"></span>
							{/if}
							<button
								type="button"
								class="remove"
								data-focus-inner
								aria-label={fill(panel.screenshots.remove, { index: i + 1 })}
								onclick={() => remove(shot.id)}
							>
								<span class="badge"><Icon icon={UTILITY_ICONS.x} size="xs" /></span>
							</button>
						</li>
					{/each}
					{#if shots.length < max}
						<li class="tile-slot" transition:fade={{ duration: 150 }}>
							<button
								type="button"
								class="add-tile"
								aria-label={panel.screenshots.add}
								onclick={() => picker?.click()}
							>
								<Icon icon={UTILITY_ICONS['image-plus']} />
							</button>
						</li>
					{/if}
				</ul>
			{/if}

			{#if refusal !== null}
				<p class="shots-note refused" role="status">
					<Icon icon={UTILITY_ICONS['triangle-alert']} />
					<span
						>{refusal === 'limit' ? panel.screenshots.limit : panel.screenshots.unsupported}</span
					>
				</p>
			{/if}
			{#if shots.length > 0}
				<!-- The founder's ruling, where it counts: before Send, and never
				     displaced by a refusal (v2 A1). -->
				<p class="shots-note public">
					<Icon icon={UTILITY_ICONS.eye} />
					<span>{panel.screenshots.public}</span>
				</p>
			{/if}
		</section>

		<Disclosure label={panel.previewToggle}>
			<dl class="env">
				{#each rows as row, i (i)}
					{#if row.label === undefined}
						<dd class="whole">{row.value}</dd>
					{:else}
						<dt>{row.label}</dt>
						<dd>
							{#each slashParts(row.value) as part, j (j)}{#if j > 0}<wbr />{/if}{part}{/each}
						</dd>
					{/if}
				{/each}
			</dl>
		</Disclosure>

		<!-- Quiet, not boxed (v2 A4): the blue-on-blue box failed 4.5:1, and a
		     promise reads as one without a tint. -->
		<p class="consent">
			<Icon icon={UTILITY_ICONS.info} />
			<span>{panel.consent}</span>
		</p>

		{#if result !== undefined && result.filed === false && result.fallbackUrl !== undefined}
			<!-- The endpoint could not file it. This is not an error message: it
			     is the other road, and the person's typing is already in that
			     URL. Title, body and the screenshots note are three parts, never
			     one glued string (v2 A2). The send button stays below as "try
			     again", because that is a real answer to a 429 or a dropped
			     connection. -->
			<div class="fallback-road" bind:this={fallbackRoad}>
				<!-- A labelled group, not an alert: focus moves to the title, and an
				     alert as well would be read twice (v3 review). -->
				<div class="fallback" role="group" aria-labelledby="{uid}-fallback">
					<Icon icon={UTILITY_ICONS['triangle-alert']} />
					<div class="fallback-text">
						<p
							id="{uid}-fallback"
							class="fallback-title"
							tabindex="-1"
							data-focus-inner
							bind:this={fallbackTitle}
						>
							{panel.fallback.title}
						</p>
						<p>{panel.fallback.body}</p>
						{#if result.withScreenshots === true || sentWithShots}
							<p class="fallback-shots">{panel.fallback.screenshots}</p>
						{/if}
					</div>
				</div>
				<Button variant="primary" shape="rounded" href={result.fallbackUrl} external
					>{panel.fallback.open}</Button
				>
			</div>
		{/if}

		<!-- Dimmed only where the button DOES something: the gallery board draws
		     ST15 as the mock draws it, at full emphasis. Busy is the spinner and
		     the sending words together, never a dimmed button (v2 A3);
		     screenshots never block it. -->
		<Button
			variant={fallen ? 'secondary' : 'primary'}
			shape="rounded"
			loading={sending || waiting}
			busyLabel={panel.sending}
			disabled={live && !ready}
			onclick={() => void send()}>{fallen ? panel.fallback.retry : panel.send}</Button
		>

		<!-- In the fallback state the block above already offers the form. -->
		{#if !fallen}
			<a
				class="github"
				href="https://github.com/mondaylabsltd/vela-wallet/issues/new"
				target="_blank"
				rel="noreferrer noopener">{panel.githubLink}</a
			>
		{/if}
	{/if}
</div>

<style>
	.feedback {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding-block: var(--space-md) var(--space-xl);
	}

	textarea {
		width: 100%;
		padding: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
		resize: vertical;
		outline: none;
	}

	textarea::placeholder {
		color: var(--color-fg-subtle);
	}

	.steps {
		align-self: flex-start;
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-info-base);
		cursor: pointer;
	}

	/* --- screenshots ---------------------------------------------------- */

	.shots {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		margin: calc(-1 * var(--space-sm));
		padding: var(--space-sm);
		border-radius: var(--radius-lg);
		transition: background var(--motion-duration-fast) ease-out;
	}

	/* A drop about to land: the section says it will take it. */
	.shots.dropping {
		background: var(--color-info-soft);
	}

	.shots-head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.shots-label {
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	/* fg-muted, not fg-subtle: subtle measured 3.38:1 on the page (v3 B8). */
	.shots-count {
		color: var(--color-fg-muted);
		font-variant-numeric: tabular-nums;
		text-align: end;
	}

	.picker {
		display: none;
	}

	.add-target,
	.add-tile {
		border: var(--border-hairline) dashed var(--color-border-strong);
		background: var(--color-bg-sunken);
		color: var(--color-fg-muted);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		cursor: pointer;
		transition: background var(--motion-duration-fast) ease-out;
	}

	/* image-plus: 20 at the standard size, growing with the words (v2 A8). */
	.add-target :global(svg),
	.add-tile :global(svg) {
		width: 1.5em;
		height: 1.5em;
	}

	.add-target {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-xs);
		width: 100%;
		min-height: var(--size-screenshotAdd);
		padding: var(--space-md) var(--space-lg);
		border-radius: var(--radius-lg);
	}

	.add-line {
		display: flex;
		align-items: center;
		gap: var(--space-md);
	}

	/* Dropping and pasting are pointer-and-keyboard habits: the hint is said
	   where a pointer is — never on a phone, where neither exists. */
	.drop-hint {
		display: none;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	@media (hover: hover) and (pointer: fine) {
		.drop-hint {
			display: block;
		}

		.add-target:hover,
		.add-tile:hover {
			background: var(--color-bg-raised);
		}
	}

	/* Five equal square columns, never a second row (v2 A6): each is 72 at
	   most and min(72, (row − 4 gaps) / 5) on a narrow screen. The padding
	   is the room the badges take past the tiles' top-trailing corners. */
	.tiles {
		display: grid;
		grid-template-columns: repeat(5, minmax(0, var(--size-screenshotTile)));
		gap: var(--space-lg);
		margin: 0;
		/* Above: the room the remove areas grow into past the tiles' tops
		   (control − badge, 22), taken back from the gap under the heading so
		   the row sits where it did. Beside: only the badge's own overhang
		   (4), so the last badge ends on the column edge, under the "n / 5"
		   counter; the last tap area reaches on into the sheet's margin, which
		   the sheet's scroll box clips sideways (BottomSheet .content) instead
		   of scrolling. */
		margin-block-start: calc(
			var(--size-screenshotBadge) - var(--size-control-md) + var(--space-sm) + var(--space-xs)
		);
		padding: 0;
		padding-block-start: calc(var(--size-control-md) - var(--size-screenshotBadge));
		padding-inline-end: var(--space-sm);
		list-style: none;
	}

	.tile,
	.tile-slot {
		position: relative;
		min-width: 0;
		aspect-ratio: 1;
	}

	/* Laid over the square rather than sizing it: the column decides the
	   tile, never the picture's own proportions. */
	.tile img,
	.processing,
	.add-tile {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}

	.tile img,
	.processing {
		display: block;
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		object-fit: cover;
		background: var(--color-bg-sunken);
	}

	/* Being prepared: the sunken tile with a small turning ring. */
	.processing::after {
		content: '';
		position: absolute;
		inset: 0;
		width: var(--icon-lg);
		height: var(--icon-lg);
		margin: auto;
		border: var(--border-emphasis) solid var(--color-border-strong);
		border-top-color: var(--color-fg-muted);
		border-radius: var(--radius-full);
		animation: spin calc(var(--motion-duration-slow) * 2) linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.add-tile {
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-lg);
	}

	/* The remove control (v2 A5, v3 B10): the button is the comfortable tap
	   area, the badge is what is seen. The area grows OUTWARD from the badge
	   — into the gap above and beside the tile — and reaches into the
	   picture only as far as the badge itself does (the badge's own size),
	   so it never takes half a small tile. The row's padding and the
	   negative margin that goes with it (.tiles) are the room it grows into,
	   so it never hangs past the column and scrolls the sheet sideways. The
	   badge sits in the area's inner corner, overhanging the tile by
	   space-sm (4). */
	.remove {
		position: absolute;
		/* Above the NEXT tile's picture, which the area reaches over into the
		   gap: later siblings paint on top otherwise, and eat the tap. */
		z-index: 1;
		top: calc(var(--size-screenshotBadge) - var(--size-control-md));
		inset-inline-end: calc(var(--size-screenshotBadge) - var(--size-control-md));
		display: flex;
		align-items: flex-start;
		justify-content: flex-end;
		width: var(--size-control-md);
		height: var(--size-control-md);
		padding: 0;
		padding-block-start: calc(
			var(--size-control-md) - var(--size-screenshotBadge) - var(--space-sm)
		);
		padding-inline-end: calc(
			var(--size-control-md) - var(--size-screenshotBadge) - var(--space-sm)
		);
		border: none;
		background: none;
		cursor: pointer;
	}

	/* OPAQUE (v3 B1): the 75% disc read two-toned over a screenshot and
	   vanished on the dark page (1.06:1). A tenth-and-a-half of the body
	   colour into the fixed ink gives the ink itself on the light page and a
	   solid neutral a step lighter than the dark page there —
	   from existing tokens, fg-base into fixed.shadowInk. White ✕, ringed in
	   the page colour. */
	.badge {
		display: flex;
		flex: none;
		align-items: center;
		justify-content: center;
		width: var(--size-screenshotBadge);
		height: var(--size-screenshotBadge);
		border-radius: var(--radius-full);
		background: color-mix(in srgb, var(--color-fg-base) 15%, var(--color-fixed-shadowInk));
		box-shadow: 0 0 0 var(--space-xs) var(--color-bg-base);
		color: var(--color-onAccent);
	}

	/* Focus rings the badge, not the invisible square around it. */
	.remove:focus-visible {
		outline: var(--space-xs) solid transparent;
	}

	.remove:focus-visible .badge {
		box-shadow:
			0 0 0 var(--space-xs) var(--color-fixed-focusRingInner),
			0 0 0 var(--space-sm) var(--color-fixed-focusRingOuter);
	}

	.shots-note {
		display: flex;
		align-items: flex-start;
		gap: var(--space-sm);
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	/* 14 at the standard size, centred on the first line whatever the scale. */
	.shots-note :global(svg),
	.consent :global(svg) {
		width: 1.25em;
		height: 1.25em;
		margin-top: calc((var(--leading-normal) * 1em - 1.25em) / 2);
	}

	.shots-note.refused {
		color: var(--color-warning-base);
	}

	/* --- what will be sent ------------------------------------------------ */

	/* Label / value rows: the label muted, the value in the body colour, and
	   a wrapped value indents under itself, not under the label. */
	.env {
		display: grid;
		grid-template-columns: fit-content(40%) minmax(0, 1fr);
		column-gap: var(--space-lg);
		row-gap: var(--space-xs);
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
	}

	.env dt {
		color: var(--color-fg-muted);
		overflow-wrap: break-word;
	}

	.env dd {
		margin: 0;
		color: var(--color-fg-base);
		overflow-wrap: break-word;
	}

	.env .whole {
		grid-column: 1 / -1;
	}

	/* --- consent, fallback ------------------------------------------------ */

	.consent {
		display: flex;
		align-items: flex-start;
		gap: var(--space-sm);
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.consent :global(svg) {
		color: var(--color-info-base);
	}

	/* The block and its button travel together: scrolled into view as one
	   when the endpoint falls back (v3 B6), with a little air around them. */
	.fallback-road {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		scroll-margin-block: var(--space-xl);
	}

	/* The fallback is a route, not a failure — amber, not red, because
	   nothing has been lost and there is a button that still works. The
	   words are in the body colour: amber on amber is a contrast failure
	   waiting to happen, and the tint and the icon already say "attention". */
	.fallback {
		display: flex;
		align-items: flex-start;
		gap: var(--space-md);
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-warning-soft);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
	}

	.fallback > :global(svg) {
		width: 1.4em;
		height: 1.4em;
		margin-top: calc((var(--leading-normal) * 1em - 1.4em) / 2);
		color: var(--color-warning-base);
	}

	.fallback-text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		min-width: 0;
	}

	.fallback-text p {
		margin: 0;
	}

	.fallback-title {
		font-weight: var(--weight-semibold);
	}

	/* Its own paragraph, a little apart: the images are a separate matter. */
	.fallback-text .fallback-shots {
		margin-top: var(--space-sm);
	}

	.github {
		align-self: center;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		text-align: center;
		/* es-MX left "reporte →" alone on the second line. */
		text-wrap: balance;
		color: var(--color-info-base);
	}

	/* --- filed ------------------------------------------------------------ */

	.filed {
		display: flex;
		flex-direction: column;
		padding-block: var(--space-2xl) var(--space-md);
	}

	/* Held at the form's height: the block sits at the 2:3 optical centre —
	   two parts of the free room above it, three below. */
	.filed.held {
		min-height: var(--held);
		padding-block: 0;
	}

	.filed.held::before,
	.filed.held::after {
		content: '';
	}

	.filed.held::before {
		flex: 2;
	}

	.filed.held::after {
		flex: 3;
	}

	.filed-block {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
		text-align: center;
	}

	/* 56, success-soft, with a thin success ring so it still reads as a disc
	   on a light page, where the soft fill is nearly the page. */
	.disc {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-emptyStateCircle);
		height: var(--size-emptyStateCircle);
		margin-bottom: var(--space-sm);
		border: var(--border-emphasis) solid
			color-mix(in srgb, var(--color-success-base) 40%, transparent);
		border-radius: var(--radius-full);
		background: var(--color-success-soft);
		color: var(--color-success-base);
	}

	.filed-block p {
		margin: 0;
		text-wrap: balance;
	}

	.filed-title {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		line-height: var(--leading-tight);
		color: var(--color-fg-base);
	}

	.filed-body,
	.filed-dropped {
		max-width: calc(var(--layout-successMeasure) * var(--text-scale, 1));
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
	}

	.filed-body {
		color: var(--color-fg-muted);
	}

	.filed-dropped {
		color: var(--color-warning-base);
	}

	/* The column's full width on a phone; on the desktop panel, a card's
	   measure — a thank-you, not a form's footer. */
	/* Focused only to be read (v3 B6/B7) — they are not controls, so no ring. */
	.filed-title:focus,
	.fallback-title:focus {
		outline: none;
	}

	/* v3 B3: the quiet way out — words, no border, the body colour; it still
	   answers a press. */
	.done {
		align-self: center;
		min-height: var(--size-control-md);
		padding-inline: var(--space-xl);
		border: none;
		border-radius: var(--radius-lg);
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-muted);
		cursor: pointer;
		transition: transform var(--motion-duration-fast) ease;
	}

	.done:hover {
		color: var(--color-fg-base);
	}

	.done:active {
		transform: scale(var(--motion-press-button));
	}

	.filed-actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		width: 100%;
		max-width: var(--layout-promptCard);
		margin-top: var(--space-lg);
	}

	@media (prefers-reduced-motion: reduce) {
		.processing::after {
			animation: none;
		}
	}
</style>
