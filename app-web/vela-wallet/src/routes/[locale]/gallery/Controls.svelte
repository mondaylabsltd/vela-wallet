<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Locale } from '$lib/i18n/locales';

	interface Props {
		locale: Locale;
		/** Current state id when on a state page; gallery root otherwise. */
		stateId?: string;
		/**
		 * A state page's fiat switch: the board as drawn, or its twin with the
		 * display currency on its way (`board-withheld.ts`). Absent on the root.
		 */
		fiatWithheld?: boolean;
		onfiat?: () => void;
	}

	let { locale, stateId, fiatWithheld = false, onfiat }: Props = $props();

	// Gallery-only appearance override: stamps the same data-theme attribute the
	// token layer already honors (tokens.css dormant overrides).
	let theme = $state<'auto' | 'light' | 'dark'>('auto');

	function cycleTheme() {
		theme = theme === 'auto' ? 'dark' : theme === 'dark' ? 'light' : 'auto';
		if (theme === 'auto') delete document.documentElement.dataset.theme;
		else document.documentElement.dataset.theme = theme;
	}

	const otherLocale = $derived(locale === 'zh' ? 'en' : 'zh');
</script>

<nav class="controls" aria-label="gallery controls">
	<a href={resolve('/[locale]/gallery', { locale })}>◱</a>
	<a
		href={stateId === undefined
			? resolve('/[locale]/gallery', { locale: otherLocale })
			: resolve('/[locale]/gallery/[state]', { locale: otherLocale, state: stateId })}
		>{otherLocale}</a
	>
	<button type="button" onclick={cycleTheme}>{theme}</button>
	{#if onfiat}
		<!-- A mark, not a word: "$" is the board as drawn, "…" its twin. -->
		<button
			type="button"
			aria-pressed={fiatWithheld}
			aria-label="fiat figures: drawn / on their way"
			data-testid="gallery-fiat"
			onclick={onfiat}>{fiatWithheld ? '…' : '$'}</button
		>
	{/if}
</nav>

<style>
	.controls {
		position: fixed;
		top: var(--space-lg);
		right: var(--space-lg);
		z-index: 1;
		display: flex;
		gap: var(--space-sm);
	}

	a,
	button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: var(--size-control-sm);
		height: var(--size-control-sm);
		padding-inline: var(--space-md);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		color: var(--color-fg-muted);
		font-family: var(--font-ui);
		font-size: var(--text-base);
		text-decoration: none;
		cursor: pointer;
	}
</style>
