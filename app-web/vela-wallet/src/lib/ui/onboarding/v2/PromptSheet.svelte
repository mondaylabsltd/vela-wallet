<script lang="ts">
	/**
	 * The one modal in the v2 flow: an interruption the person has to answer.
	 *
	 * The whole journey is a full page; only FAILURES are modal, because a
	 * failure genuinely does stop everything until it is acknowledged. Below the
	 * desktop breakpoint it is a bottom sheet with a drag handle; above it, a
	 * centred card.
	 *
	 * `confirmable` is the core's word for "this answer changes the flow" — the
	 * recovery offer is the only prompt where declining is a decision rather
	 * than a dismissal, and it is the only one that gets two real buttons.
	 */
	import { MediaQuery } from 'svelte/reactivity';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { BREAKPOINT_DESKTOP } from '$lib/tokens/tokens';
	import type { PromptCopy } from '$lib/onboarding/core/copy';

	interface Props {
		copy: PromptCopy;
		dismissLabel: string;
		/** true = accepted, false = declined or dismissed. */
		onAnswer: (accepted: boolean) => void;
	}

	let { copy, dismissLabel, onAnswer }: Props = $props();

	const desktop = new MediaQuery(`(min-width: ${BREAKPOINT_DESKTOP}px)`, false);
	let sheet = $state<{ requestClose: () => void }>();

	/** A dismissal is a refusal — the core reads it as `accepted: false`. */
	let answer = false;

	function accept() {
		answer = true;
		close();
	}

	function decline() {
		answer = false;
		close();
	}

	function close() {
		if (sheet) sheet.requestClose();
		else onAnswer(answer);
	}
</script>

{#snippet body()}
	<div class="prompt">
		<h2 class="title">{copy.title}</h2>
		<p class="message">{copy.message}</p>
		<div class="actions">
			{#if copy.confirm}
				<Button variant="primary" shape="rounded" onclick={accept}
					>{copy.confirm.confirmLabel}</Button
				>
				<Button variant="secondary" shape="rounded" onclick={decline}
					>{copy.confirm.cancelLabel}</Button
				>
			{:else}
				<Button variant="primary" shape="rounded" onclick={decline}>{dismissLabel}</Button>
			{/if}
		</div>
	</div>
{/snippet}

{#if desktop.current}
	<div
		class="scrim"
		role="dialog"
		aria-modal="true"
		aria-label={copy.title}
		tabindex="-1"
		onkeydown={(event) => event.key === 'Escape' && decline()}
	>
		<div class="card">{@render body()}</div>
	</div>
{:else}
	<BottomSheet
		bind:this={sheet}
		title={copy.title}
		hideTitle
		variant="prompt"
		onclose={() => onAnswer(answer)}
	>
		{@render body()}
	</BottomSheet>
{/if}

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 10;
		display: grid;
		place-items: center;
		padding: var(--space-3xl);
		background: var(--color-fixed-backdrop);
	}

	.card {
		width: 100%;
		max-width: var(--layout-promptCard);
		padding: var(--space-3xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-xl);
		background: var(--color-bg-raised);
		box-shadow: var(--shadow-lg);
	}

	.prompt {
		display: flex;
		flex-direction: column;
		gap: var(--space-2xl);
	}

	.title {
		margin: 0;
		color: var(--color-fg-base);
		font-size: var(--text-xl);
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
		letter-spacing: -0.01em;
	}

	.message {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: var(--text-base);
		line-height: var(--leading-relaxed);
		white-space: pre-line;
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}
</style>
