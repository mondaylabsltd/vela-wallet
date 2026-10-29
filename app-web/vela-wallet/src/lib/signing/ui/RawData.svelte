<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import { copyText } from '$lib/services/clipboard';
	import WarningBanner from './WarningBanner.svelte';
	import { capLines, rawView, type RawLine, type RawView } from '../raw-json';
	import type { RawDataModel } from '../model';

	/**
	 * Technical details' last layer: the request as the site sent it, laid
	 * out to be read (`raw-json.ts`) and copied as it came. The text is the
	 * site's — every run below is a text node, never markup.
	 */
	interface Props {
		raw: RawDataModel;
	}

	let { raw }: Props = $props();

	/*
	 * Laid out once per text: the sheet's model is rebuilt on every fee tick,
	 * and a 200 KB request laid out again each time is time for nothing.
	 */
	let laid: { hex: string; jsonText: string; view: RawView } | undefined;
	const view = $derived.by(() => {
		const { hex } = raw;
		const { jsonText } = raw.words;
		if (laid?.hex !== hex || laid.jsonText !== jsonText) {
			laid = { hex, jsonText, view: rawView(hex, { jsonText }) };
		}
		return laid.view;
	});
	/** A long payload asks before it is drawn whole: the sheet opens at once. */
	let whole = $state(false);
	const capped = $derived(whole ? { lines: view.lines, cut: false } : capLines(view.lines));

	let copied = $state(false);
	let copiedTimer: ReturnType<typeof setTimeout> | undefined;
	$effect(() => () => clearTimeout(copiedTimer));

	/** The ORIGINAL string, byte for byte — never the lines drawn. */
	async function copy(): Promise<void> {
		if (!(await copyText(raw.hex))) return;
		copied = true;
		clearTimeout(copiedTimer);
		copiedTimer = setTimeout(() => (copied = false), 1500);
	}

	const indent = (line: RawLine) => Math.min(line.depth, 12);
</script>

<div class="raw-data">
	<div class="head">
		<p class="layer-label">{raw.label}</p>
		<button
			type="button"
			class="copy"
			data-testid="raw-copy"
			aria-label={raw.words.copy}
			title={raw.words.copy}
			onclick={copy}
		>
			<Icon icon={UTILITY_ICONS[copied ? 'check' : 'copy']} size="sm" />
			{#if copied}<span class="copied" aria-hidden="true">{raw.words.copied}</span>{/if}
		</button>
		<!-- Read, not seen: the same word, for whoever cannot see it land. -->
		<span class="visually-hidden" role="status">{copied ? raw.words.copied : ''}</span>
	</div>

	<!-- Lines are blocks, so a wrapped line keeps its indent; the runs are
	     text, so nothing a site writes is ever markup. -->
	<div class="code" class:json={view.json} data-testid="raw-code" dir="ltr" translate="no">
		{#each capped.lines as line, i (i)}
			<div class="line" class:hex={line.hex} style:--depth={indent(line)}>
				{#each line.runs as run, j (j)}<span class={run.kind}>{run.text}</span>{/each}
			</div>
		{/each}
	</div>

	{#if capped.cut}
		<button type="button" class="show-all" onclick={() => (whole = true)}>
			{raw.words.showAll[0]}{view.lines.length}{raw.words.showAll[1]}
		</button>
	{/if}

	{#if view.hidden}
		<WarningBanner tone="danger" text={raw.words.hidden} />
	{/if}
</div>

<style>
	.raw-data {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		min-width: 0;
	}

	.head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-md);
	}

	.layer-label {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	/* Copying moves nothing: neutral ink, the app's copy glyph and its word. */
	.copy {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		padding: var(--space-sm);
		margin: calc(var(--space-sm) * -1);
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.copy:focus-visible {
		outline: var(--space-xs) solid var(--color-fg-base);
		outline-offset: var(--space-xs);
	}

	.copied {
		font-family: var(--font-ui);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.code {
		margin: 0;
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-relaxed);
		color: var(--color-fg-muted);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		unicode-bidi: isolate;
	}

	.line {
		padding-inline-start: calc(var(--depth) * 2ch);
	}

	/* The pieces of a long hex value: a breath between words, so one that
	   wraps on a phone still reads as one word. */
	.line.hex + .line.hex {
		margin-top: var(--space-sm);
	}

	/* Neutral inks only (AA on bg.sunken, `contrast.test.ts`): the values in
	   the strongest, the names a step back, the structure the faintest — its
	   meaning is also the layout. */
	.json .key {
		color: var(--color-fg-muted);
	}

	.json .string {
		color: var(--color-fg-base);
	}

	.json .number,
	.json .literal {
		color: var(--color-fg-base);
		font-weight: var(--weight-medium);
	}

	.json .punct {
		color: var(--color-fg-subtle);
	}

	.plain {
		color: var(--color-fg-muted);
	}

	/* What the wallet put there for the eye, never mistaken for the text: a
	   hidden character's ⟨U+…⟩ — in full ink, for it is the warning — and the
	   note over a string read as JSON. */
	.hidden {
		color: var(--color-fg-base);
		font-style: italic;
		text-decoration: underline dotted;
	}

	.note {
		font-family: var(--font-ui);
		font-style: italic;
		color: var(--color-fg-muted);
	}

	.show-all {
		align-self: flex-start;
		padding: var(--space-sm) 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
		text-decoration: underline;
		cursor: pointer;
	}

	.show-all:focus-visible {
		outline: var(--space-xs) solid var(--color-fg-base);
		outline-offset: var(--space-xs);
	}
</style>
