<script lang="ts">
	/**
	 * ST15 — the in-app report, asked for as help (round 3, owner: "像测试工单，
	 * 不像求助的地方"). The first screen is one box and a Send: a sentence is
	 * enough. Everything else is said only when it is needed —
	 *
	 * - **Steps** have no box of their own: the one box covers them. A report
	 *   seeded with steps (a relay stop's) carries them in its words
	 *   (`ReportDraft.seed`), so the payload's `steps` goes empty.
	 * - **Screenshots** are a small neutral "Attach a screenshot"; tiles appear
	 *   once there is one, and the cap is said only when it is reached.
	 * - **What will be sent** is still sent, every line of it, and still shown
	 *   — folded into one quiet line, "Version and device info are included ·
	 *   View". View opens the rows, and the consent ("only what is shown;
	 *   never your passkeys") opens with them: the promise stays next to the
	 *   thing it is a promise about.
	 * - **GitHub** is not on the first screen: the thank-you offers the issue
	 *   the report became, and a report that could not be sent offers the
	 *   prefilled form (below).
	 *
	 * Spec 081 FR-016 gave the button somewhere to go; 078 round 3 (founder,
	 * 2026-09-26: "方便把截图上传，多张截图也要能上传") adds screenshots:
	 *
	 * - **Up to five**, picked (the hidden input behind both add targets),
	 *   pasted anywhere while the sheet is open, or dropped on the sheet.
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
	 *
	 * A tile is a button (078 §C): it opens the screenshot large — the
	 * PROCESSED JPEG, what goes public — in `ScreenshotViewer`, and closing
	 * the viewer puts focus back on that tile (or, if it was removed there, on
	 * the tile now in its place, else the attach button). A tile still being
	 * prepared opens nothing, and nothing opens while the form is sending.
	 *
	 * What was typed and attached lives in a `ReportDraft`. The route hands in
	 * the app-resident one, so closing the sheet — even mid-send — never loses
	 * the report; with none handed in (the gallery, the tests) the sheet keeps
	 * its own and discards it when it goes.
	 */
	import { onDestroy, onMount, tick, untrack } from 'svelte';
	import { fade, scale } from 'svelte/transition';
	import type { FeedbackModel, FeedbackResult } from '../model';
	import { ReportDraft } from '../report-draft.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { fill } from '$lib/wallet/messages';
	import { imageFilesOf, SCREENSHOT_ACCEPT } from '$lib/services/screenshot-prep';
	import ScreenshotViewer, { type ViewerImage } from './ScreenshotViewer.svelte';
	import { tapGuard } from './tap-guard';

	interface Props {
		panel: FeedbackModel;
		/**
		 * Absent in the gallery, where this sheet is a picture of itself.
		 * `screenshots` are the processed JPEGs' base64, in tile order.
		 */
		onsend?: (report: { what: string; screenshots: string[] }) => void;
		/** The route is waiting on the endpoint. */
		sending?: boolean;
		/** The last send's outcome, or `undefined` before the first. */
		result?: FeedbackResult;
		/** 完成 on the filed state: the host closes the sheet (or resets the panel). */
		ondone?: () => void;
		/**
		 * The report being written. The route passes the app-resident one so it
		 * outlives this sheet; absent, the sheet keeps a draft of its own.
		 */
		draft?: ReportDraft;
	}

	let { panel, onsend, sending = false, result, ondone, draft: given }: Props = $props();

	/** Fixed for this sheet's life: a draft is never swapped under a form. */
	const draft = untrack(() => given) ?? new ReportDraft();
	const ownDraft = untrack(() => given) === undefined;

	let dropping = $state(false);
	/** Whether the send in flight carried images — the fallback says they cannot follow. */
	let sentWithShots = $state(false);
	let picker = $state<HTMLInputElement | undefined>();

	const max = $derived(panel.screenshots.max);
	/** Five attached: said once, when it is true — never as a hint before. */
	const full = $derived(draft.shots.length >= max);
	/**
	 * Words are what make a report: the endpoint refuses one without a
	 * description (`missing_description`), so a screenshot alone does not
	 * enable Send — it would only fall back to the form.
	 */
	const ready = $derived(draft.what.trim() !== '');
	/** The diagnostics' rows (and the consent with them), unfolded. */
	let detailsOpen = $state(false);
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

	let root = $state<HTMLDivElement | undefined>();
	let filedTitle = $state<HTMLElement | undefined>();
	let fallbackTitle = $state<HTMLElement | undefined>();
	let fallbackRoad = $state<HTMLElement | undefined>();

	/** Sending, or waiting on a tile before sending: the form holds still (v3 B9). */
	const busy = $derived(sending || draft.waiting);

	/**
	 * Each outcome is said where the person is (v3 B6/B7). Filed: focus moves
	 * to the thank-you's title, so a screen reader reads it and a keyboard is
	 * not dropped on the page body. Fell back: the block AND its "Open the
	 * GitHub form" button are scrolled into view — on a real-height phone they
	 * landed below the fold — and focus goes to the block's title.
	 *
	 * When the outcome ARRIVES while this sheet is open, and — for the
	 * fallback — when the sheet is reopened on it (078 review, B6): a person
	 * who closed the sheet mid-send and was told by a toast that it did not
	 * go comes back to the form's road in view, not to the top of the form
	 * with the button below the fold. A thank-you reopened grabs nothing.
	 */
	const outcomeAtMount = untrack(() => result);
	/** Names the outcome blocks for assistive tech (the title is also where focus goes). */
	const uid = $props.id();
	/**
	 * The thank-you, said (round 3): focus lands on its title, which a screen
	 * reader reads; this polite region, mounted from the start so a change in
	 * it is announced, says the rest. Reopened on a thank-you, it starts full
	 * and says nothing new.
	 */
	const announced = $derived(
		result?.filed === true
			? [
					panel.success.body,
					...((result.screenshotsDropped ?? 0) > 0 ? [panel.success.dropped] : [])
				].join(' ')
			: ''
	);
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
	function scrollerOf(el: HTMLElement): HTMLElement | null {
		let node = el.parentElement;
		while (node !== null && !/(auto|scroll)/.test(getComputedStyle(node).overflowY)) {
			node = node.parentElement;
		}
		return node;
	}

	/**
	 * Bring `el` into view inside the sheet (or pane) that scrolls it — by
	 * scrolling THAT, and nothing else. `scrollIntoView` would also scroll
	 * every clipping ancestor, and a sheet still sliding in counts as
	 * overflow of the screen under it.
	 */
	function reveal(el: HTMLElement): void {
		const scroller = scrollerOf(el);
		if (scroller === null) {
			el.scrollIntoView({ block: 'nearest' });
			return;
		}
		const box = el.getBoundingClientRect();
		const frame = scroller.getBoundingClientRect();
		const margin = parseFloat(getComputedStyle(el).scrollMarginBottom) || 0;
		const below = box.bottom + margin - frame.bottom;
		if (below > 0) scroller.scrollTop += below;
	}

	onMount(() => {
		const outcome = outcomeAtMount;
		if (outcome === undefined || root === undefined) return;
		if (!outcome.filed && fallbackRoad !== undefined) {
			fallbackTitle?.focus({ preventScroll: true });
			reveal(fallbackRoad);
		}
	});

	/** Take files from any of the three doors: the picker, a paste, a drop. */
	function add(files: readonly File[]): void {
		if (!live || busy || files.length === 0) return;
		draft.add(files, max);
	}

	function remove(id: number): void {
		draft.remove(id);
	}

	/**
	 * The badge and the tile act on a TAP only: a scroll that starts on a
	 * badge — its tap area reaches out to where a thumb starts scrolling the
	 * sheet — must scroll, never delete (iPhone, 2026-09-27).
	 */
	const removeTap = tapGuard();
	const viewTap = tapGuard();

	// --- the viewer (078 §C) ---------------------------------------------------

	/** The tiles that can open: prepared, i.e. showing the bytes that will be sent. */
	const viewable = $derived<ViewerImage[]>(
		draft.shots.flatMap((shot, i) =>
			shot.url === undefined
				? []
				: [{ id: shot.id, url: shot.url, label: fill(panel.screenshots.view, { index: i + 1 }) }]
		)
	);
	/** The image the viewer opened on, while it is open. */
	let viewing = $state<number | null>(null);
	/**
	 * The tile focus goes back to when the viewer closes: the one that opened
	 * it — or, when that one was removed in the viewer, the tile that took its
	 * place (`null`: none did, so the add target).
	 */
	let returnTo: number | null = null;
	let tilesList = $state<HTMLElement | undefined>();

	function view(id: number): void {
		if (!live || busy) return;
		if (!viewable.some((image) => image.id === id)) return;
		returnTo = id;
		viewing = id;
	}

	function removeFromViewer(id: number): void {
		if (returnTo === id) {
			const at = draft.shots.findIndex((s) => s.id === id);
			returnTo = draft.shots[at + 1]?.id ?? null;
		}
		remove(id);
	}

	/**
	 * Focus back on the tile (or the add target). The ring shows only when the
	 * viewer was closed from the keyboard; a tap or click that closed it
	 * leaves focus in place without drawing a ring nobody asked for.
	 */
	async function viewerClosed(via: 'keyboard' | 'pointer'): Promise<void> {
		viewing = null;
		await tick();
		const tile =
			returnTo === null
				? null
				: (tilesList?.querySelector<HTMLElement>(`[data-shot="${returnTo}"] .view`) ?? null);
		// The attach button first: when the last tile went, the row (and its
		// add tile) is still leaving.
		const target =
			tile ??
			root?.querySelector<HTMLElement>('.attach') ??
			root?.querySelector<HTMLElement>('.add-tile') ??
			null;
		target?.focus({ preventScroll: true, focusVisible: via === 'keyboard' } as FocusOptions);
	}

	function picked(event: Event & { currentTarget: HTMLInputElement }): void {
		const input = event.currentTarget;
		add(Array.from(input.files ?? []));
		// Picking the same file again must fire `change` again.
		input.value = '';
	}

	/** Paste anywhere in the sheet: images only — text keeps pasting as text. */
	function pasted(event: ClipboardEvent): void {
		if (result?.filed === true || busy || viewing !== null) return;
		const files = imageFilesOf(event.clipboardData).filter((f) => f.type.startsWith('image/'));
		if (files.length === 0) return;
		event.preventDefault();
		add(files);
	}

	/** A drop anywhere on the sheet attaches — not on the thank-you. */
	function dropped(event: DragEvent): void {
		if (!event.dataTransfer?.types.includes('Files')) return;
		event.preventDefault();
		dropping = false;
		if (result?.filed === true) return;
		add(imageFilesOf(event.dataTransfer));
	}

	function dragOver(event: DragEvent): void {
		if (!live || result?.filed === true || !event.dataTransfer?.types.includes('Files')) return;
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
		if (draft.waiting) return;
		// A send is the next change: the refusal has been read. (A tile that
		// fails while this send waits says so again.)
		draft.refusal = null;
		// Read before the wait: a sheet closed during it takes its props with it.
		const deliver = onsend;
		if (draft.preparing) {
			draft.waiting = true;
			try {
				await draft.settled();
			} finally {
				draft.waiting = false;
			}
		}
		// NOT abandoned when the sheet closed meanwhile: the draft outlives it,
		// and the person pressed Send.
		const screenshots = draft.screenshots();
		sentWithShots = screenshots.length > 0;
		deliver?.({ what: draft.what, screenshots });
	}

	function done(): void {
		draft.reset();
		ondone?.();
	}

	onDestroy(() => {
		if (ownDraft) draft.reset();
	});
</script>

<svelte:window onpaste={pasted} />

<!-- The whole sheet takes a drop (round 3): the attach button is small now,
     and a screenshot dragged onto the words should land too. -->
<div
	class="feedback"
	class:dropping
	bind:this={root}
	role="group"
	ondragover={dragOver}
	ondragleave={() => (dropping = false)}
	ondrop={dropped}
>
	{#if result?.filed === true}
		{@const dropped = (result.screenshotsDropped ?? 0) > 0}
		<!-- Filed: a thank-you, warm and short. The issue it became is a quiet
		     link under it — a person who reported something is owed a way back
		     to it, and when images were lost, that page is where they go. Its
		     own height, no more (round 3): the phone sheet hugs it rather than
		     keeping the form's height, and the desktop pane shows it under the
		     title, not at the foot of a tall empty frame. No accent: nothing
		     here moves value. -->
		<div class="filed" in:fade={{ duration: 150 }}>
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
				<p class="filed-body">{panel.success.body}</p>
				{#if dropped}
					<p class="filed-dropped">{panel.success.dropped}</p>
				{/if}
				{#if result.url !== undefined}
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- the issue on GitHub, never an app route -->
					<a class="github" href={result.url} target="_blank" rel="noreferrer noopener"
						>{panel.success.followUp}</a
					>
				{/if}
				<div class="filed-actions">
					<!-- The one action here, in the neutral primary: an outlined Done
					     in muted ink read as disabled. -->
					<Button variant="primary" shape="rounded" onclick={done}>{panel.success.done}</Button>
				</div>
			</div>
		</div>
	{:else}
		<!-- One box (round 3): three rows to start, growing with what it holds. -->
		<textarea
			data-field
			inert={busy}
			bind:value={draft.what}
			placeholder={panel.placeholder}
			aria-label={panel.label}
			rows="3"
			style:--rows="3"></textarea>

		<!-- The screenshots. Paste works from anywhere in the sheet (the window
		     listener above), a drop anywhere on it (the root). Inert while
		     sending (v3 B9): what is being sent does not change under the
		     spinner. -->
		<div class="shots" inert={busy}>
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

			{#if draft.shots.length === 0}
				<div class="attach-row">
					<button type="button" class="attach" onclick={() => picker?.click()}>
						<Icon icon={UTILITY_ICONS['image-plus']} />
						<span>{panel.screenshots.add}</span>
					</button>
					<span class="drop-hint">{panel.screenshots.dropHint}</span>
				</div>
			{:else}
				<!-- One row, always (v2 A6): five equal columns that shrink
				     together on a narrow screen instead of wrapping a lone tile
				     onto a second line. -->
				<ul class="tiles" aria-label={panel.screenshots.label} bind:this={tilesList}>
					{#each draft.shots as shot, i (shot.id)}
						<li class="tile" data-shot={shot.id} transition:scale={{ duration: 150, start: 0.9 }}>
							{#if shot.url !== undefined}
								<!-- The tile's body opens it large (§C1); the badge above it
								     keeps removing. -->
								<button
									type="button"
									class="view"
									aria-label={fill(panel.screenshots.view, { index: i + 1 })}
									onpointerdown={viewTap.down}
									onpointermove={viewTap.move}
									onpointercancel={viewTap.cancel}
									onclick={(event) => {
										if (viewTap.accept(event)) view(shot.id);
									}}
								>
									<img src={shot.url} alt="" />
								</button>
							{:else}
								<!-- Being prepared: nothing to show yet, so nothing opens. -->
								<span class="processing" aria-hidden="true"></span>
							{/if}
							<button
								type="button"
								class="remove"
								data-focus-inner
								aria-label={fill(panel.screenshots.remove, { index: i + 1 })}
								onpointerdown={removeTap.down}
								onpointermove={removeTap.move}
								onpointercancel={removeTap.cancel}
								onclick={(event) => {
									if (removeTap.accept(event)) remove(shot.id);
								}}
							>
								<span class="badge"><Icon icon={UTILITY_ICONS.x} size="xs" /></span>
							</button>
						</li>
					{/each}
					{#if !full}
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

			{#if draft.refusal !== null}
				<p class="shots-note refused" role="status">
					<Icon icon={UTILITY_ICONS['triangle-alert']} />
					<span
						>{draft.refusal === 'limit'
							? panel.screenshots.limit
							: panel.screenshots.unsupported}</span
					>
				</p>
			{:else if full}
				<!-- The cap, said when it is reached and not before (round 3). A
				     sixth refused says it in its own words above instead. -->
				<p class="shots-note full">{panel.screenshots.full}</p>
			{/if}
			{#if draft.shots.length > 0}
				<!-- The founder's ruling, where it counts: before Send, and never
				     displaced by a refusal (v2 A1). -->
				<p class="shots-note public">
					<Icon icon={UTILITY_ICONS.eye} />
					<span>{panel.screenshots.public}</span>
				</p>
			{/if}
		</div>

		<!-- What will be sent: every line still goes, and is still shown — on
		     asking (round 3). The consent opens with the rows it is about. -->
		<div class="details">
			<p class="details-line">
				<span id="{uid}-details-line">{panel.details.included}</span>
				<span aria-hidden="true">·</span>
				<button
					type="button"
					class="details-toggle"
					aria-expanded={detailsOpen}
					aria-controls="{uid}-details"
					aria-describedby="{uid}-details-line"
					onclick={() => (detailsOpen = !detailsOpen)}
					>{detailsOpen ? panel.details.hide : panel.details.view}</button
				>
			</p>
			<div id="{uid}-details" class="details-body" hidden={!detailsOpen}>
				<dl class="env">
					{#each rows as row, i (i)}
						{#if row.label === undefined}
							<dd class="whole">{row.value}</dd>
						{:else}
							<dt>{row.label}</dt>
							<dd>{row.value}</dd>
						{/if}
					{/each}
				</dl>
				<!-- Quiet, not boxed (v2 A4), and neutral: a promise reads as one
				     without a tint or a coloured icon. -->
				<p class="consent">
					<Icon icon={UTILITY_ICONS.info} />
					<span>{panel.consent}</span>
				</p>
			</div>
		</div>

		{#if result !== undefined && result.filed === false && result.fallbackUrl !== undefined}
			<!-- The endpoint could not file it — said so, plainly, with the words
			     still in the box. It is also the other road: the person's typing
			     is already in that URL. Title, body and the screenshots note are
			     three parts, never one glued string (v2 A2). The send button
			     stays below as "try again", because that is a real answer to a
			     429 or a dropped connection. -->
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

		<!-- The neutral primary: a report moves no value, so never the accent.
		     Dimmed only where the button DOES something: the gallery board
		     draws ST15 at full emphasis. Busy is the spinner and the sending
		     words together, never a dimmed button (v2 A3); screenshots never
		     block it. -->
		<Button
			variant={fallen ? 'secondary' : 'primary'}
			shape="rounded"
			loading={busy}
			busyLabel={panel.sending}
			disabled={live && !ready}
			onclick={() => void send()}>{fallen ? panel.fallback.retry : panel.send}</Button
		>
	{/if}

	<p class="visually-hidden" role="status">{announced}</p>

	<!-- Mounted on `viewing` alone: when its last image is removed the viewer
	     plays its exit and reports back, which is when focus is placed. -->
	{#if viewing !== null}
		<ScreenshotViewer
			images={viewable}
			startId={viewing}
			closeLabel={panel.screenshots.close}
			removeLabel={panel.screenshots.removeFromViewer}
			onremove={removeFromViewer}
			onclose={(via) => void viewerClosed(via)}
		/>
	{/if}
</div>

<style>
	.feedback {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding-block: var(--space-md) var(--space-xl);
		border-radius: var(--radius-lg);
		transition: background var(--motion-duration-fast) ease-out;
	}

	/* A drop about to land anywhere on the sheet: it says it will take it —
	   in the neutral sunken tone, never the info blue. */
	.feedback.dropping {
		background: var(--color-bg-sunken);
		outline: var(--border-emphasis) dashed var(--color-border-strong);
		outline-offset: var(--space-xs);
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
		/* As tall as what it holds, never shorter than its rows: a report
		   seeded from a relay stop (issue 466) is a title, the treasury's whole
		   address and its figures — past four rows at phone width — and read in
		   a box that scrolls inside a sheet that scrolls, half of it was hidden. */
		field-sizing: content;
		box-sizing: border-box;
		min-block-size: calc(
			var(--rows) * var(--text-base) * var(--text-scale, 1) * var(--leading-normal) + 2 *
				(var(--space-lg) + var(--border-hairline))
		);
	}

	textarea::placeholder {
		color: var(--color-fg-subtle);
	}

	/* --- screenshots ---------------------------------------------------- */

	.shots {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.shots:empty {
		display: none;
	}

	.picker {
		display: none;
	}

	/* Round 3: a small neutral button, not a dashed drop zone — the words
	   come first, a screenshot is an extra. */
	.attach-row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-sm) var(--space-md);
	}

	.attach {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		min-height: var(--size-control-sm);
		padding: 0 var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-full);
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
		transition:
			background var(--motion-duration-fast) ease-out,
			transform var(--motion-duration-fast) ease;
	}

	.attach:active {
		transform: scale(var(--motion-press-button));
	}

	/* image-plus beside the words, growing with them (v2 A8). */
	.attach :global(svg) {
		width: 1.25em;
		height: 1.25em;
		color: var(--color-fg-muted);
	}

	.add-tile {
		border: var(--border-hairline) dashed var(--color-border-strong);
		background: var(--color-bg-sunken);
		color: var(--color-fg-muted);
		cursor: pointer;
		transition: background var(--motion-duration-fast) ease-out;
	}

	.add-tile :global(svg) {
		width: 1.5em;
		height: 1.5em;
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

		.attach:hover,
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
		   (control − badge, 22), taken back from the gap above so the row
		   sits close under the box. Beside: only the badge's own overhang
		   (4), so the last badge ends on the column edge; the last tap area
		   reaches on into the sheet's margin, which
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
	.view,
	.processing,
	.add-tile {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}

	/* The tile's body is the button that opens it large (078 §C1). */
	.view {
		display: block;
		padding: 0;
		border: none;
		border-radius: var(--radius-lg);
		background: none;
		cursor: zoom-in;
		transition: transform var(--motion-duration-fast) ease;
	}

	.view:focus-visible {
		border-radius: var(--radius-lg);
	}

	.view:active {
		transform: scale(var(--motion-press-button));
	}

	@media (hover: hover) and (pointer: fine) {
		.view:hover img {
			opacity: var(--opacity-hover);
		}
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
			0 0 0 var(--space-sm) var(--color-fg-base);
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

	/* One quiet line: what is included, and the word that shows it. */
	.details-line {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		column-gap: var(--space-sm);
		min-height: var(--size-control-sm);
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	/* vis-21: a text action carries the neutral ink, underlined — never the
	   info blue. */
	.details-toggle {
		min-height: var(--size-control-sm);
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: inherit;
		color: var(--color-fg-base);
		text-decoration: underline;
		text-underline-offset: var(--space-xs);
		cursor: pointer;
	}

	.details-body {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		padding-top: var(--space-sm);
	}

	.details-body[hidden] {
		display: none;
	}

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
		color: var(--color-fg-muted);
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

	/* The title in the body colour; what follows in fg-muted (5.1:1 on the
	   amber tint), as iOS and Android draw it. */
	.fallback-text p {
		margin: 0;
		color: var(--color-fg-muted);
	}

	.fallback-text .fallback-title {
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	/* Its own paragraph, a little apart and a step smaller: the images are a
	   separate, lesser matter. */
	.fallback-text .fallback-shots {
		margin-top: var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.github {
		align-self: center;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		text-align: center;
		/* es-MX left "reporte →" alone on the second line. */
		text-wrap: balance;
		/* vis-21: the neutral ink every link and text action carries. */
		color: var(--color-fg-base);
		text-decoration: underline;
		text-underline-offset: var(--space-xs);
	}

	/* --- filed ------------------------------------------------------------ */

	.filed {
		display: flex;
		flex-direction: column;
		padding-block: var(--space-2xl) var(--space-md);
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
