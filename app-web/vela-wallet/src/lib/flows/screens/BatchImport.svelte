<script lang="ts">
	/**
	 * SD2c — bringing a list of people to pay.
	 *
	 * The screen reads top to bottom as the job does: say what the amounts are
	 * IN, bring the list (paste it, pick a file, or take the template first),
	 * check the rate that turns the sheet's figures into the token, read each
	 * person's share back against the sheet, and see what it adds up to against
	 * what the account holds. The button waits at the bottom of the screen —
	 * pinned, because a payroll is long — with the total above it.
	 *
	 * Bad rows are marked and skipped, never silently dropped, and the CTA
	 * counts only the good ones — a button that says "Import 3" and imports 2
	 * is how someone underpays a contractor.
	 *
	 * Whatever stops the import is said where the eye is when the button will
	 * not press (issue 204): the total turns to the refusal colour beside the
	 * balance it exceeds, with the sentence under it.
	 */
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import GhostPillRow from '../ui/GhostPillRow.svelte';
	import MonoField from '../ui/MonoField.svelte';
	import SegmentedToggle from '../ui/SegmentedToggle.svelte';
	import type { BatchImportModel } from '../model';

	interface Props {
		model: BatchImportModel;
		onunit?: (id: string) => void;
		onfile?: () => void;
		ontemplate?: () => void;
		onapply?: () => void;
		/** Present ⇒ the rate can be typed in place (spec 038 E6). */
		onrate?: (text: string) => void;
		onresetrate?: () => void;
		/**
		 * Present ⇒ the table can be pasted here (spec 026). Absent, the field
		 * stays the drawn picture the gallery renders.
		 */
		onpaste?: (value: string) => void;
		/** Add to the people already on the form, or replace them: the other one. */
		onmerge?: () => void;
	}

	let { model, onunit, onfile, ontemplate, onapply, onpaste, onrate, onresetrate, onmerge }: Props =
		$props();

	const tools = $derived([
		{
			id: 'file',
			label: model.tools.file.label,
			icon: 'upload' as const,
			busy: model.tools.file.busy
		},
		{
			id: 'template',
			label: model.tools.template.label,
			icon: model.tools.template.saved ? ('check' as const) : ('download' as const),
			done: model.tools.template.saved
		}
	]);
</script>

<div class="batch">
	<div class="unit">
		<p class="caption">{model.unitCaption}</p>
		<SegmentedToggle
			label={model.unitCaption}
			selected={model.unit}
			options={[
				{ id: 'fiat', label: model.units.fiat },
				{ id: 'token', label: model.units.token }
			]}
			onselect={onunit}
		/>
	</div>

	<div class="source">
		<MonoField
			value={model.pasteValue}
			placeholder={model.pastePlaceholder}
			rows={5}
			oninput={onpaste}
		/>
		<GhostPillRow items={tools} onselect={(id) => (id === 'file' ? onfile?.() : ontemplate?.())} />
		<p class="formats" class:named={model.tools.fileName !== undefined}>
			{model.tools.fileName ?? model.tools.formats}
		</p>
		{#if model.tools.error !== undefined}
			<p class="refusal" role="alert">
				<Icon icon={UTILITY_ICONS['circle-alert']} size="sm" />
				<span>{model.tools.error}</span>
			</p>
		{/if}
	</div>

	{#if model.rate !== undefined}
		{@const rate = model.rate}
		<section class="rate">
			<div class="rate-row">
				<span class="rate-label">{rate.section}</span>
				<span class="equation" class:live={rate.editable && onrate !== undefined}>
					<span class="side">{rate.lead}</span>
					<span class="side sign">{rate.sign}</span>
					{#if rate.editable && onrate}
						<input
							class="figure"
							inputmode="decimal"
							autocomplete="off"
							aria-label={`${rate.section} · ${rate.lead} ${rate.sign} ${rate.code}`}
							value={rate.value}
							oninput={(event) => onrate(event.currentTarget.value)}
						/>
					{:else}
						<span class="figure">{rate.value}</span>
					{/if}
					<span class="side">{rate.code}</span>
				</span>
				{#if rate.edited && onresetrate}
					<button type="button" class="reset" onclick={onresetrate}>
						<Icon icon={UTILITY_ICONS['rotate-ccw']} size="sm" />
						{rate.reset}
					</button>
				{:else}
					<span class="pencil" aria-hidden="true">
						<Icon icon={UTILITY_ICONS.pencil} size="sm" />
					</span>
				{/if}
			</div>
			<p class="hint" class:warning={rate.hintTone === 'warning'}>{rate.hint}</p>
		</section>
	{/if}

	{#if model.unitHint !== undefined}
		<p class="hint unit-hint">{model.unitHint}</p>
	{/if}

	{#if model.preview !== undefined}
		<section class="preview">
			<h3>{model.preview.label}</h3>
			<ul>
				{#each model.preview.rows as row, i (i)}
					{#if row.kind === 'refused'}
						<!-- A line that never became a row: no address to draw, nobody to
						     name — the text to find it by in the sheet, and the reason. -->
						<li class="skipped refused">
							<span class="art none" aria-hidden="true">
								<Icon icon={UTILITY_ICONS.x} size="sm" />
							</span>
							<span class="who"><span class="written" title={row.text}>{row.text}</span></span>
							<span class="note">{row.note}</span>
						</li>
					{:else}
						<li class:skipped={!row.ok}>
							<span class="art">
								<Identicon svg={row.identiconSvg} size="row" address={row.addressFull} />
							</span>
							<span class="who">
								{#if row.name !== undefined}
									<span class="name">{row.name}</span>
								{/if}
								<span class="address" class:lead={row.name === undefined} title={row.addressFull}>
									{row.address}
								</span>
							</span>
							{#if row.note !== undefined}
								<span class="note">{row.note}</span>
							{:else}
								<span class="figures">
									<span class="amount">{row.amount}</span>
									{#if row.source !== undefined}
										<span class="from">{row.source}</span>
									{/if}
								</span>
							{/if}
						</li>
					{/if}
				{/each}
			</ul>
		</section>
	{/if}

	{#if model.notices.length > 0}
		<ul class="notices">
			{#each model.notices as notice (notice)}
				<li>
					<Icon icon={UTILITY_ICONS['triangle-alert']} size="sm" />
					<span>{notice}</span>
				</li>
			{/each}
		</ul>
	{/if}

	<div class="footer">
		{#if model.total !== undefined}
			{@const total = model.total}
			<div class="total" class:over={total.over}>
				<p class="line">
					<span class="total-label">{total.label}</span>
					<span class="total-value">{total.value}</span>
				</p>
				<p class="line sub">
					<span>{total.balance}</span>
					{#if total.detail !== undefined}<span>{total.detail}</span>{/if}
				</p>
				{#if total.overText !== undefined}
					<p class="refusal" role="alert">
						<Icon icon={UTILITY_ICONS['circle-alert']} size="sm" />
						<span>{total.overText}</span>
					</p>
				{/if}
			</div>
		{/if}
		{#if model.merge !== undefined}
			<p class="merge">
				<Icon icon={UTILITY_ICONS.info} size="sm" />
				<span>
					{model.merge.note}
					{#if onmerge}
						<button type="button" class="swap" onclick={onmerge}>{model.merge.action}</button>
					{/if}
				</span>
			</p>
		{/if}
		<Button variant="primary" shape="rounded" disabled={model.ctaDisabled} onclick={onapply}>
			{model.cta}
		</Button>
	</div>
</div>

<style>
	.batch {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
	}

	p,
	h3,
	ul {
		margin: 0;
		padding: 0;
	}

	ul {
		list-style: none;
	}

	/* --- what the figures are --------------------------------------------- */

	.unit {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.caption {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	/* --- bringing the list ------------------------------------------------ */

	.source {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	/* What the picker takes, or — once something was picked — which file the
	   rows below came from: a workbook leaves the paste box empty. */
	.formats {
		text-align: center;
		font-family: var(--font-mono);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		letter-spacing: var(--letterSpacing-sectionLabel);
		color: var(--color-fg-subtle);
		overflow-wrap: anywhere;
	}

	.named {
		letter-spacing: normal;
		color: var(--color-fg-muted);
	}

	.refusal {
		display: flex;
		align-items: flex-start;
		gap: var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-error-base);
	}

	.refusal :global(svg),
	.notices :global(svg) {
		flex-shrink: 0;
		margin-block-start: var(--space-xs);
	}

	/* --- the rate --------------------------------------------------------- */

	.rate {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		padding-block-start: var(--space-xl);
		border-block-start: var(--border-hairline) solid var(--color-border-base);
	}

	.rate-row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-sm) var(--space-md);
	}

	.rate-label {
		flex: 1;
		min-width: max-content;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	/* One sentence — "1 BNB ≈ 752.7 USD" — in one face, with the part that can
	   be typed over underlined. It used to be a label, then a bare input in
	   the browser's own font, then nothing: the unit was never drawn. */
	.equation {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-sm);
		min-width: 0;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}

	.side {
		white-space: nowrap;
		color: var(--color-fg-muted);
	}

	.figure {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}

	input.figure {
		field-sizing: content;
		min-width: 4ch;
		max-width: 14ch;
		padding: var(--space-xs) 0;
		border: none;
		border-block-end: var(--border-hairline) solid var(--color-border-strong);
		border-radius: 0;
		background: none;
		text-align: center;
	}

	input.figure:focus {
		outline: none;
		border-block-end-color: var(--color-fg-base);
	}

	.pencil {
		display: flex;
		color: var(--color-fg-subtle);
	}

	.reset {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		padding: var(--space-xs) var(--space-md);
		border: none;
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		font-family: var(--font-ui);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.reset:active {
		transform: scale(var(--motion-press-button));
	}

	.hint {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.warning {
		color: var(--color-warning-base);
	}

	/* Token mode has no rate block to sit in, so the sentence draws its own rule. */
	.unit-hint {
		padding-block-start: var(--space-xl);
		border-block-start: var(--border-hairline) solid var(--color-border-base);
	}

	/* --- who gets what ---------------------------------------------------- */

	.preview {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	h3 {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		letter-spacing: var(--letterSpacing-sectionLabel);
		text-transform: uppercase;
		color: var(--color-fg-subtle);
	}

	.preview li {
		position: relative;
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-lg);
		padding-block: var(--space-md);
	}

	/* A hairline between people, set in past the artwork so it runs under the
	   text it divides. */
	.preview li + li::before {
		content: '';
		position: absolute;
		inset-block-start: 0;
		inset-inline: calc(var(--icon-2xl) + var(--space-lg)) 0;
		block-size: var(--border-hairline);
		background: var(--color-border-base);
	}

	.art {
		display: flex;
	}

	.who,
	.figures {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		min-width: 0;
	}

	.name {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-base);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	/* No name in the sheet: the address IS who, so it takes the name's place. */
	.address.lead {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.figures {
		align-items: flex-end;
		text-align: end;
	}

	.amount {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}

	.from {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-muted);
	}

	/* A refused line has no artwork: a cross in the artwork's seat, so the
	   text still starts where every name starts. */
	.none {
		align-items: center;
		justify-content: center;
		width: var(--icon-2xl);
		height: var(--icon-2xl);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		color: var(--color-fg-muted);
	}

	/* The refused line's TEXT is what the person reads to find it in the sheet,
	   so it does not step back with the rest of a skipped row. */
	.skipped.refused .who {
		opacity: 1;
	}

	.written {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	/* A skipped row stays on the list — it was in the sheet — but steps back,
	   and says why in the colour for "look at this". */
	.skipped .art,
	.skipped .who {
		opacity: var(--opacity-dim);
	}

	.note {
		max-width: 16ch;
		text-align: end;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-warning-base);
	}

	.notices {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.notices li {
		display: flex;
		align-items: flex-start;
		gap: var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-warning-base);
	}

	/* --- the total and the button, pinned --------------------------------- */

	.footer {
		position: sticky;
		inset-block-end: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		padding-block: var(--space-lg) var(--space-md);
		background: var(--color-bg-base);
		border-block-start: var(--border-hairline) solid var(--color-border-base);
	}

	.total {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.line {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-xs) var(--space-lg);
	}

	.total-label {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.total-value {
		/* Wrapped under a long label (German, at the largest text), the figure
		   keeps to the edge every other figure on the screen is set against. */
		margin-inline-start: auto;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}

	.over .total-value {
		color: var(--color-error-base);
	}

	.sub {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-muted);
	}

	.merge {
		display: flex;
		align-items: flex-start;
		gap: var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.merge :global(svg) {
		flex-shrink: 0;
		margin-block-start: var(--space-xs);
	}

	/* The other choice, in the sentence that states this one: a link's weight,
	   never the accent — it changes what the button below will do, it does not
	   do it. */
	.swap {
		padding: 0;
		border: none;
		background: none;
		font: inherit;
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		text-decoration: underline;
		text-underline-offset: var(--space-xs);
		cursor: pointer;
	}

	.total .refusal {
		padding-block-start: var(--space-xs);
	}
</style>
