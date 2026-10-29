<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';

	interface Props {
		tone: 'caution' | 'danger';
		text: string;
		/** The line that lands after the sheet has opened (the relay's verdict): named, so the sheet can keep it in sight. */
		verdict?: boolean;
		/**
		 * Drawn as ROOM, not as a line (`VerdictPlace`): the same banner, laid
		 * out at the same size and never said. Its words are generated content,
		 * so nothing of a sentence the sheet has not said is in the page's text
		 * — not for a reader, not for a find, not for a test that asks whether
		 * the sheet says it.
		 */
		room?: boolean;
		/**
		 * dapp-15: the line is about the network, not the transaction —
		 * "Check again" (a button), or, while a new check is out, the line
		 * marked stale with "Checking again…".
		 */
		recheck?: { label: string; checking: boolean };
		onrecheck?: () => void;
	}

	let { tone, text, verdict = false, room = false, recheck, onrecheck }: Props = $props();
</script>

<div
	class="banner"
	data-tone={tone}
	data-verdict={verdict ? '' : undefined}
	role={tone === 'danger' && !room ? 'alert' : undefined}
>
	<Icon icon={UTILITY_ICONS['triangle-alert']} size="base" />
	{#if room}
		<p data-room={text}></p>
	{:else}
		<!-- Not `.said`: that is the verdict place's own cell (`VerdictPlace`),
		     which this banner is drawn inside — one name, one element. -->
		<div class="words" class:stale={recheck?.checking === true}>
			<p>{text}</p>
			{#if recheck?.checking}
				<p class="again" role="status" aria-busy="true">{recheck.label}</p>
			{:else if recheck && onrecheck}
				<button type="button" class="again" data-tap onclick={onrecheck}>{recheck.label}</button>
			{/if}
		</div>
	{/if}
</div>

<style>
	.banner {
		display: flex;
		align-items: flex-start;
		gap: var(--space-lg);
		padding: var(--space-lg) var(--space-xl);
		border-radius: var(--radius-lg);
		border: var(--border-hairline) solid transparent;
	}

	[data-tone='caution'] {
		background: var(--color-warning-soft);
		border-color: var(--color-warning-border);
		color: var(--color-warning-base);
	}

	[data-tone='danger'] {
		background: var(--color-error-soft);
		border-color: var(--color-error-base);
		color: var(--color-error-base);
	}

	.words {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		min-width: 0;
	}

	/* The line a new check is about to replace: still there, plainly not current. */
	.stale > p:first-child {
		opacity: var(--opacity-disabled);
	}

	.again {
		align-self: flex-start;
		padding: 0;
		border: 0;
		background: none;
		color: inherit;
		font: inherit;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		text-decoration: underline;
		cursor: pointer;
	}

	p.again {
		margin: 0;
		text-decoration: none;
		cursor: default;
	}

	p {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
	}

	/* A room's words: laid out exactly as the sentence would be, and no text. */
	p[data-room]::before {
		content: attr(data-room);
	}
</style>
