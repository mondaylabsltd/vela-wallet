<script lang="ts">
	/**
	 * The keys that control this wallet, and their record's copy on Ethereum
	 * (spec 062).
	 *
	 * One block on both layouts. A key row says three things a person can act
	 * on: what it is called, WHO is holding it (the vault's own mark and name
	 * when the core's catalog knows — Apple Passwords, 1Password, a security
	 * key), and whether it is synced or lives on one device. The copy's row
	 * sits under the keys it is about, as their last row, instead of floating
	 * on the page as a sentence about nothing in particular.
	 *
	 * De-containered, hairline-divided, like the rest of settings: these are
	 * facts to read, not cards to tap. Only the copy's row is a button, and
	 * only while a tap does something — which the core says (`action`): `copy`
	 * opens the signing sheet, `retry` asks again, `none` is a statement.
	 *
	 * NOTHING UNDER IT MOVES for what the chains answer (device walk, item
	 * 13.1). The device's own record gives the rows from the first frame;
	 * while the registry is being asked (`model.asking`) a row says only what
	 * that record knows — the key's name and short id, and whether this device
	 * signs with it — and draws the rest as quiet room: who holds it, its mark,
	 * the badges. A row keeps the room of the fullest badges it can end with,
	 * and the copy's row the room of its tallest second line, so each is one
	 * height before the answer, after it, and when there is none.
	 */
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import PasskeyProviderMark from '$lib/ui/onboarding/v2/PasskeyProviderMark.svelte';
	import { providerLabel } from '$lib/onboarding/core/passkey-directory.svelte';
	import { isDarkTheme } from '$lib/theme.svelte';
	import type { WalletKeysModel } from '../model';

	interface Props {
		model: WalletKeysModel;
		onbackup?: () => void;
	}

	let { model, onbackup }: Props = $props();

	/** Which rows are open. By position: the list is the founding order. */
	let open = $state<Record<number, boolean>>({});
	/** `"row:label"` of the value just copied, for the button's one-second "Copied". */
	let copied = $state('');
	let copiedTimer: ReturnType<typeof setTimeout> | undefined;

	async function copy(id: string, value: string) {
		try {
			await navigator.clipboard.writeText(value);
			copied = id;
			clearTimeout(copiedTimer);
			copiedTimer = setTimeout(() => (copied = ''), 1200);
		} catch {
			// No clipboard permission: the value is on screen and selectable.
		}
	}
</script>

<section class="keys" aria-label={model.title}>
	<header>
		<h2>
			{model.title}
			{#if model.count !== ''}<span class="count">{model.count}</span>{/if}
		</h2>
		<p class="subtitle">{model.subtitle}</p>
	</header>

	{#if model.loading}
		<!-- The shape of one row, for the instant before the device's record is read. -->
		<div class="row skeleton" aria-hidden="true">
			<span class="mark-slot"></span>
			<span class="who"><span class="bar wide"></span><span class="bar"></span></span>
		</div>
	{:else}
		<ul aria-busy={model.asking}>
			{#each model.rows as row, index (index)}
				<!-- Captioned by where the key lives (spec 102: three places, no fourth). -->
				{@const holder =
					providerLabel(row.key.provider_name, row.key.aaguid, isDarkTheme()) ?? row.holderFallback}
				{@const expandable = row.details.length > 0}
				{@const isOpen = open[index] === true && !model.asking}
				<!-- While asking, the fullest set stands in, quietly, for the badges to come. -->
				{@const shown = model.asking ? (row.pillRoom[0] ?? row.pills) : row.pills}
				<li class="key" class:open={isOpen}>
					<!--
						The row is the summary; opening it shows what the registry holds
						about this key — the explorer's facts, here where the person is
						deciding to back them up. Nothing to open when only the device
						answered, and then it is not a button.
					-->
					{#snippet summary()}
						<!--
							The mark and the holder's words are the registry's to give:
							the device reads where a key lives off its stored transports
							alone, and showed "Built-in passkey" for the seconds before
							the registry said "Security key". Until then, their room.
						-->
						<span class="mark-slot" class:quiet={model.asking}>
							{#if !model.asking}
								<PasskeyProviderMark key={row.key} label={holder} glyphFallback />
							{/if}
						</span>
						<span class="who">
							<span class="name"
								>{#if row.name === ''}<span class="ghost long" aria-hidden="true">&nbsp;</span
									>{:else}{row.name}{/if}</span
							>
							<span class="meta">
								{#if model.asking}<span class="ghost" aria-hidden="true">&nbsp;</span
									>{:else}{holder}{/if}{#if row.fingerprint !== ''}<span
										class="sep"
										aria-hidden="true">·</span
									><span class="fingerprint">{row.fingerprint}</span>{/if}
							</span>
						</span>
						<span class="pills">
							<!-- The room: every set this row can end with, unseen, in the one cell. -->
							{#each row.pillRoom as set, at (at)}
								<span class="set room" aria-hidden="true">
									{#each set as pill (pill.text)}
										<span class="pill-room" data-tone={pill.tone}
											>{#if pill.tone === 'signs_here'}<Icon
													icon={UTILITY_ICONS.check}
													size="xs"
												/>{/if}{pill.text}</span
										>
									{/each}
								</span>
							{/each}
							<span class="set">
								{#each shown as pill (pill.text)}
									<!-- Which key this device signs with is the device's own to say. -->
									{@const quiet = model.asking && pill.tone !== 'signs_here'}
									<span
										class="pill"
										class:quiet
										data-tone={pill.tone}
										aria-hidden={quiet || undefined}
										>{#if pill.tone === 'signs_here'}<Icon
												icon={UTILITY_ICONS.check}
												size="xs"
											/>{/if}{#if quiet}<span class="room">{pill.text}</span
											>{:else}{pill.text}{/if}</span
									>
								{/each}
							</span>
						</span>
					{/snippet}
					{#if expandable}
						<!--
							The same element while asking, so the row is the same box —
							but there is nothing of the registry's to open yet.
						-->
						<button
							type="button"
							class="row summary"
							aria-expanded={isOpen}
							disabled={model.asking}
							onclick={() => (open[index] = !open[index])}
						>
							{@render summary()}
							<span class="chevron"><Icon icon={UTILITY_ICONS['chevron-down']} size="sm" /></span>
						</button>
					{:else}
						<div class="row summary">{@render summary()}</div>
					{/if}

					{#if expandable && isOpen}
						<dl class="details">
							{#each row.details as detail (detail.label)}
								{@const id = `${index}:${detail.label}`}
								<div class="detail">
									<dt>{detail.label}</dt>
									<dd>
										<span class="value" class:mono={detail.mono}>{detail.value}</span>
										{#if detail.copy}
											<button type="button" class="copy" onclick={() => copy(id, detail.value)}>
												{copied === id ? model.copy.done : model.copy.action}
											</button>
										{/if}
									</dd>
								</div>
							{/each}
						</dl>
					{/if}
				</li>
			{/each}
		</ul>
		{#if model.note}<p class="note">{model.note}</p>{/if}
		<!-- Spec 102: a wallet on its own signing domain says so once — its keys
		     answer only on that domain's page. -->
		{#if model.domain}<p class="note domain">{model.domain}</p>{/if}
	{/if}

	{#if model.backup}
		{@const backup = model.backup}
		<button
			type="button"
			class="row backup"
			data-tone={backup.tone}
			data-action={backup.action}
			disabled={backup.action === 'none'}
			onclick={() => onbackup?.()}
		>
			<span class="mark-slot"><Icon icon={UTILITY_ICONS.upload} size="md" /></span>
			<span class="who">
				<span class="name">{backup.title}</span>
				<!--
					One height through "Checking…" and its answer: every line the
					row can say is laid in the one cell, unseen, under the one it
					is saying.
				-->
				<span class="lines">
					{#each model.backupLines as line (line)}
						<span class="meta room" aria-hidden="true">{line}</span>
					{/each}
					<span class="meta state">{backup.subtitle}</span>
				</span>
			</span>
			<!-- …and one width: the glyph's room is kept when there is no glyph. -->
			{#if backup.action === 'retry'}
				<Icon icon={UTILITY_ICONS['refresh-cw']} size="sm" />
			{:else if backup.action === 'copy'}
				<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
			{:else if backup.tone === 'positive'}
				<span class="done"><Icon icon={UTILITY_ICONS.check} size="sm" /></span>
			{:else}
				<span class="glyph-room" aria-hidden="true"></span>
			{/if}
		</button>
		<!--
			The core names the paragraph per state and names none for a record
			that can never be copied (PR 3 note 6): then nothing is drawn here,
			and no room is kept — the block ends on its row.
		-->
		{#if backup.explain !== undefined}
			<p class="explain">{backup.explain}</p>
		{/if}
	{/if}
</section>

<style>
	.keys {
		display: flex;
		flex-direction: column;
		font-family: var(--font-ui);
	}

	header {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		margin-bottom: var(--space-md);
	}

	h2 {
		display: flex;
		align-items: baseline;
		gap: var(--space-md);
		margin: 0;
		color: var(--color-fg-base);
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
	}

	.count {
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-regular);
		font-variant-numeric: tabular-nums;
	}

	.subtitle,
	.note {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-relaxed);
	}

	.note {
		padding-block: var(--space-md);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding-block: var(--space-lg);
	}

	.mark-slot {
		display: grid;
		flex: none;
		place-items: center;
		/* The settings rows' own icon column, so names line up with the page. */
		width: var(--icon-2xl);
		height: var(--space-4xl);
		color: var(--color-fg-muted);
	}

	/*
		The name keeps room of its own. With none, long pills (ru: "Вход
		выполнен", "Подтверждение при использовании", "Привязан к устройству")
		squeezed it to nothing and laid over the holder line — the pills wrap
		first, and the holder · fingerprint line may wrap under the name.
	*/
	.who {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-xs);
		min-width: 9em;
		/* Its own `em`, whichever element the row is: a button's is the UA's. */
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	/*
		Beside the pills (the wide layout) the name keeps a holder line's worth:
		"Google Password Manager · 197d…647b" on ONE line. At 9em it was squeezed
		there whenever the badges were long (ru), and the holder line wrapped to
		two or three — which a row cannot keep room for before the registry has
		said who the holder is. The pills wrap first, in room the row does keep.
	*/
	@media (min-width: 1280px) {
		.key .who {
			min-width: min(100%, 21em);
		}
	}

	/* Wraps, never "…" (078 round 3): "Respaldar las llaves pública…" was the
	   backup row's own title cut short on a phone at the standard size. */
	.name {
		color: var(--color-fg-base);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		overflow-wrap: break-word;
	}

	.meta {
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.sep {
		margin-inline: var(--space-sm);
	}

	.fingerprint {
		font-family: var(--font-mono);
	}

	.key {
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.summary {
		border: none;
		border-bottom: none;
		background: none;
		padding-inline: 0;
		color: inherit;
		font-family: var(--font-ui);
		text-align: start;
	}

	button.summary {
		cursor: pointer;
	}

	/* Still asking: the row is not opened yet, and does not look as if it could be. */
	button.summary:disabled {
		cursor: default;
	}

	button.summary:disabled .chevron {
		visibility: hidden;
	}

	/*
		Shrinks to its widest pill and wraps, end-aligned, before the name gives
		way. One cell holding every set the row can end with (`.room`, unseen)
		under the set it shows: the cell is as wide and as tall as the fullest.
	*/
	.pills {
		display: grid;
		flex: 0 1 auto;
	}

	.set {
		display: flex;
		grid-area: 1 / 1;
		flex-wrap: wrap;
		align-content: flex-start;
		justify-content: flex-end;
		gap: var(--space-sm);
		min-width: 0;
	}

	.room {
		visibility: hidden;
	}

	/*
		Below the desktop breakpoint there is no room for a name AND two pills on
		one line — the name lost ("Parallel O…"). The pills take their own line
		under the text, and the chevron stays beside the name.
	*/
	@media not (min-width: 1280px) {
		.summary {
			flex-wrap: wrap;
			row-gap: var(--space-md);
		}

		.chevron {
			order: 2;
		}

		.pills {
			order: 3;
			flex-basis: 100%;
			padding-inline-start: calc(var(--icon-2xl) + var(--space-lg));
		}

		.set {
			justify-content: flex-start;
		}
	}

	/* The explorer's pills: a hairline capsule, tinted by what it vouches for.
	   `.pill-room` is the same box, unseen: what a row's room is made of. */
	.pill,
	.pill-room {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		padding: var(--space-xs) var(--space-md);
		border: var(--border-hairline) solid currentColor;
		border-radius: var(--radius-full);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		white-space: nowrap;
	}

	/*
		The key this device signs with (founder, 2026-09-26: it must stand out).
		The one FILLED pill, first in the row; the hairline stays, invisible, so
		it is exactly as tall as the outlined ones beside it. Success, not accent
		— accent is for money and for submitting.
	*/
	.pill[data-tone='signs_here'] {
		border-color: transparent;
		background: var(--color-success-soft);
		color: var(--color-success-base);
		font-weight: var(--weight-medium);
	}

	/* Its weight is part of its width. */
	.pill-room[data-tone='signs_here'] {
		font-weight: var(--weight-medium);
	}

	/*
		A badge nobody has vouched for yet: where it will stand, and no word —
		its words are only its width (`.room`: not drawn, not read, not found by
		a search of the page).
	*/
	.pill.quiet {
		border-color: transparent;
		background: var(--color-border-base);
	}

	.pill[data-tone='verified'] {
		color: var(--color-info-base);
	}

	.pill[data-tone='synced'] {
		color: var(--color-success-base);
	}

	.pill[data-tone='local'] {
		color: var(--color-fg-muted);
	}

	.chevron {
		display: grid;
		flex: none;
		place-items: center;
		color: var(--color-fg-muted);
		transition: transform var(--motion-fast, 120ms) ease-out;
	}

	.open .chevron {
		transform: rotate(180deg);
	}

	@media (prefers-reduced-motion: reduce) {
		.chevron {
			transition: none;
		}
	}

	.details {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		margin: 0;
		padding: var(--space-xl);
		margin-bottom: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
	}

	.detail {
		display: grid;
		grid-template-columns: minmax(0, 7.5em) minmax(0, 1fr);
		gap: var(--space-lg);
		align-items: start;
	}

	dt {
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		letter-spacing: 0.06em;
		text-transform: uppercase;
		padding-top: var(--space-xs);
	}

	dd {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-md);
		margin: 0;
		min-width: 0;
	}

	.value {
		min-width: 0;
		color: var(--color-fg-base);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		overflow-wrap: anywhere;
	}

	.value.mono {
		font-family: var(--font-mono);
		/* A key is compared by eye; give the glyphs room. */
		line-height: var(--leading-relaxed);
	}

	.copy {
		flex: none;
		padding: var(--space-xs) var(--space-md);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-md);
		background: none;
		color: var(--color-fg-muted);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		cursor: pointer;
	}

	.copy:hover {
		color: var(--color-fg-base);
	}

	.explain {
		margin: 0;
		/* Under the backup's text, not under its icon. */
		padding-inline-start: calc(var(--icon-2xl) + var(--space-lg));
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-relaxed);
	}

	/* The backup: the block's last row, and its only button. */
	.backup {
		padding-inline: 0;
		border: none;
		border-bottom: none;
		color: inherit;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.backup:disabled {
		cursor: default;
	}

	/* The one tone that is not the row's own: copied. "Not copied yet" is a
	   state, not a defect — it is optional and costs a fee — so it is never
	   the warning colour (the core's `BackupTone` has no caution). */
	.backup[data-tone='positive'] .state,
	.done {
		color: var(--color-success-base);
	}

	.backup:not(:disabled):hover .name {
		color: var(--color-accent-base);
	}

	.skeleton .bar {
		display: block;
		width: 30%;
		height: var(--space-md);
		border-radius: var(--radius-full);
		background: var(--color-border-base);
	}

	.skeleton .bar.wide {
		width: 45%;
	}

	.skeleton .mark-slot,
	.mark-slot.quiet {
		border-radius: var(--radius-full);
		background: var(--color-border-base);
	}

	/*
		Words still to come, as a bar on their own line: an inline box holding a
		space, so the line is exactly as tall as the words will make it.
	*/
	.ghost {
		display: inline-block;
		width: 7em;
		border-radius: var(--radius-full);
		background: var(--color-border-base);
		line-height: 1;
		user-select: none;
	}

	.ghost.long {
		width: 9em;
	}

	/* The copy row's second line, and every line it could be instead. */
	.lines {
		display: grid;
	}

	.lines > * {
		grid-area: 1 / 1;
	}

	/* A trailing glyph's box (`Icon`'s `sm`) with no glyph in it. */
	.glyph-room {
		flex: none;
		width: var(--icon-sm);
		height: var(--icon-sm);
	}
</style>
