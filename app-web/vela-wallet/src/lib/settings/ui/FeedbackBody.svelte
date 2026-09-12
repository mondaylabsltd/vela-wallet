<script lang="ts">
	/**
	 * ST15 — the in-app report. The disclosure showing exactly what will be
	 * sent is open by default and the consent note sits directly above the send
	 * button, because the promise ("never keys, seed phrase, or balances") is
	 * only worth anything next to the thing it is a promise about.
	 */
	import type { FeedbackModel } from '../model';
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Disclosure from './Disclosure.svelte';

	interface Props {
		panel: FeedbackModel;
		onsend?: () => void;
	}

	let { panel, onsend }: Props = $props();
</script>

<div class="feedback">
	<textarea data-field placeholder={panel.placeholder} aria-label={panel.placeholder} rows="4"
	></textarea>

	<button type="button" class="steps">{panel.addSteps}</button>

	<Disclosure label={panel.previewToggle}>
		{#each panel.previewLines as line (line)}
			<p class="line">{line}</p>
		{/each}
	</Disclosure>

	<p class="consent">
		<Icon icon={UTILITY_ICONS.info} size="md" />
		<span>{panel.consent}</span>
	</p>

	<Button variant="primary" shape="rounded" onclick={onsend}>{panel.send}</Button>

	<a
		class="github"
		href="https://github.com/mondaylabsltd/vela-wallet/issues/new"
		target="_blank"
		rel="noreferrer noopener">{panel.githubLink}</a
	>
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

	.line {
		margin: 0;
	}

	.consent {
		display: flex;
		align-items: flex-start;
		gap: var(--space-md);
		margin: 0;
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-info-soft);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-info-base);
	}

	.github {
		align-self: center;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-info-base);
	}
</style>
