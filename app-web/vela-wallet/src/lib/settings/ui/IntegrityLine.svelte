<script lang="ts">
	/**
	 * What is trusted about a signing page, in one line (spec 102 R6): "Version
	 * 0ba8ee8c · matches Vela's published build list · checked 14:32", or why it
	 * will not be opened.
	 *
	 * The word "trusted" is only ever backed by THIS: a version a person can
	 * read and compare, what it was checked against, and when. Never "certified"
	 * or "untampered" — the check proves the page is a build Vela published, not
	 * that nothing anywhere could have changed it.
	 *
	 * The version is set in the mono face so it reads as the hash it is, and a
	 * refusal leads with a warning glyph: a line that is red only by colour is a
	 * line some people cannot see is red. A check still running is a clock, not
	 * a warning; a self-hosted build asking to be trusted is a caution (amber
	 * glyph, quiet words), never drawn as the refusal it is not.
	 *
	 * The line keeps two lines' room — what a verdict takes — so a check
	 * landing ("Checking the page…" → "Version … · checked 14:32") never moves
	 * what is under it (the hand-off card's Open).
	 */
	import type { IntegrityLineModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	let { line }: { line: IntegrityLineModel } = $props();

	/** The eight hex characters of a version, set apart so the mono face can hold them. */
	const parts = $derived.by(() => {
		const match = /\b[0-9a-f]{8}\b/.exec(line.text);
		if (match === null) return { before: line.text, version: '', after: '' };
		return {
			before: line.text.slice(0, match.index),
			version: match[0],
			after: line.text.slice(match.index + match[0].length)
		};
	});
</script>

<p class="integrity" data-tone={line.tone}>
	<span class="glyph" aria-hidden="true">
		<Icon
			icon={line.tone === 'ok'
				? UTILITY_ICONS['shield-check']
				: line.tone === 'checking'
					? UTILITY_ICONS.clock
					: UTILITY_ICONS['triangle-alert']}
			size="sm"
		/>
	</span>
	<span class="text"
		>{parts.before}{#if parts.version !== ''}<span class="version">{parts.version}</span
			>{/if}{parts.after}</span
	>
</p>

<style>
	.integrity {
		display: flex;
		align-items: flex-start;
		gap: var(--space-sm);
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.glyph {
		display: flex;
		flex-shrink: 0;
		/* Centre the glyph on the FIRST line of a line that may wrap. */
		padding-block-start: calc((var(--text-sm) * var(--leading-normal) - var(--icon-sm)) / 2);
	}

	.integrity[data-tone='ok'] .glyph {
		color: var(--color-success-base);
	}

	.integrity[data-tone='checking'] .glyph {
		color: var(--color-fg-subtle);
	}

	.integrity[data-tone='warn'] .glyph {
		color: var(--color-warning-base);
	}

	/* A refusal is said in the line's own colour too: it will not open. */
	.integrity[data-tone='error'] {
		color: var(--color-error-base);
	}

	.text {
		min-width: 0;
		/* Two lines of its own size, held whatever the line says. */
		min-block-size: 2lh;
		overflow-wrap: anywhere;
	}

	.version {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}

	.integrity[data-tone='error'] .version {
		color: inherit;
	}
</style>
