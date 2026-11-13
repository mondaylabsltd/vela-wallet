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
	 * recovery offer and the free retry of a lookup nobody answered are the
	 * prompts where declining is a decision rather than a dismissal, and they
	 * are the ones that get two real buttons.
	 */
	import { MediaQuery } from 'svelte/reactivity';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { dialogPop, scrimFade } from '$lib/ui/dialog-motion';
	import { BREAKPOINT_DESKTOP } from '$lib/tokens/tokens';
	import type { PromptCopy } from '$lib/onboarding/core/copy';

	interface Props {
		copy: PromptCopy;
		dismissLabel: string;
		/** true = accepted, false = declined or dismissed. */
		onAnswer: (accepted: boolean) => void;
		/** The confirm takes no tap yet (an agent's request just up). */
		confirmDisabled?: boolean;
		/**
		 * The refusal leads: first and primary, the confirm quiet under it —
		 * for a question whose safe answer is no (an agent's request).
		 */
		refuseFirst?: boolean;
	}

	let {
		copy,
		dismissLabel,
		onAnswer,
		confirmDisabled = false,
		refuseFirst = false
	}: Props = $props();

	const desktop = new MediaQuery(`(min-width: ${BREAKPOINT_DESKTOP}px)`, false);
	let sheet = $state<{ requestClose: () => void }>();
	/**
	 * The desktop card takes focus when it opens, so its Escape (on the scrim)
	 * is heard: opened from a button elsewhere, the keys went to that button
	 * and the card could only be closed with the mouse.
	 */
	let scrim = $state<HTMLElement>();
	$effect(() => {
		if (scrim && !scrim.contains(document.activeElement)) scrim.focus({ preventScroll: true });
	});

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
			{#if copy.confirm && refuseFirst}
				<Button variant="primary" shape="rounded" onclick={decline}
					>{copy.confirm.cancelLabel}</Button
				>
				<Button variant="secondary" shape="rounded" disabled={confirmDisabled} onclick={accept}
					>{copy.confirm.confirmLabel}</Button
				>
			{:else if copy.confirm}
				<Button variant="primary" shape="rounded" disabled={confirmDisabled} onclick={accept}
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
		bind:this={scrim}
		onkeydown={(event) => event.key === 'Escape' && decline()}
		transition:scrimFade|global
	>
		<!-- vis-19: the card comes and goes with the motion every dialog has. -->
		<div class="card" transition:dialogPop|global>{@render body()}</div>
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

	/* The size every sheet's title is drawn at (BottomSheet's h2): a prompt
	   ("Network space", "Sign out?") read a step smaller than the sheets
	   around it (vis-19). */
	.title {
		margin: 0;
		color: var(--color-fg-base);
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
		letter-spacing: -0.01em;
	}

	.message {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-relaxed);
		white-space: pre-line;
		/* A name or an address with no break in it wraps rather than running
		   off the sheet. */
		overflow-wrap: anywhere;
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}
</style>
