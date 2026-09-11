<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		/**
		 * `danger` is spec 023's addition: 退出登录 / 仍然退出 / 全部清除 are
		 * filled buttons in the error colour, not accent ones. Accent is reserved
		 * for the action that moves value (design review 2026-07), and signing out
		 * or erasing a device moves none — it destroys.
		 */
		variant: 'primary' | 'secondary' | 'danger';
		/**
		 * `pill` is the Welcome page's shape (spec 006); `rounded` is the v2
		 * onboarding flow's, whose design draws rectangles at `--radius-lg`
		 * throughout. A prop rather than a second button: the two differ in
		 * one radius, and a fork would make every later change to states,
		 * sizing or disabled treatment happen twice.
		 */
		shape?: 'pill' | 'rounded';
		/** Renders an <a> when set (and not disabled), else a <button>. */
		href?: string;
		/** With `href`: a page outside the app (an explorer) — a new tab, no opener. */
		external?: boolean;
		disabled?: boolean;
		/**
		 * The action is running and this button is what the person is waiting
		 * on. Deliberately not the same as `disabled`: a dimmed button reads as
		 * "unavailable", which is the one thing "working" must never look like.
		 * The button keeps full emphasis, holds its size, and turns a spinner
		 * where its label was (docs/design-system.md — "Loading state:
		 * ActivityIndicator replacing text").
		 */
		loading?: boolean;
		onclick?: () => void;
		children: Snippet;
	}

	let {
		variant,
		shape = 'pill',
		href,
		external = false,
		disabled = false,
		loading = false,
		onclick,
		children
	}: Props = $props();
</script>

{#if href !== undefined && !disabled && !loading}
	<!-- eslint-disable svelte/no-navigation-without-resolve -- generic component; callers pass resolve()d paths, or an external URL -->
	<a
		class="button {variant} {shape}"
		{href}
		target={external ? '_blank' : undefined}
		rel={external ? 'noreferrer noopener' : undefined}>{@render children()}</a
	>
	<!-- eslint-enable svelte/no-navigation-without-resolve -->
{:else}
	<button
		class="button {variant} {shape}"
		class:loading
		disabled={disabled || loading}
		aria-busy={loading}
		{onclick}
		type="button"
	>
		<!-- Hidden rather than removed: the label goes on holding the button's
		     width and height, so the spinner's arrival reflows nothing. -->
		<span class="label">{@render children()}</span>
		{#if loading}<span class="spinner" aria-hidden="true"></span>{/if}
	</button>
{/if}

<style>
	.button {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		/* Minimum, not fixed: long-locale labels wrap and grow the pill
		   instead of overflowing it (spec 014 long-label fix). */
		min-height: var(--size-control-lg);
		min-width: var(--size-control-md);
		padding-inline: var(--space-3xl);
		padding-block: var(--space-md);
		border: none;
		font-family: var(--font-ui);
		font-size: var(--text-xl);
		font-weight: var(--weight-semibold);
		line-height: var(--leading-tight);
		text-align: center;
		text-decoration: none;
		cursor: pointer;
		user-select: none;
		transition:
			opacity var(--motion-duration-fast) ease,
			transform var(--motion-duration-fast) ease;
	}

	.pill {
		border-radius: var(--radius-full);
	}

	.rounded {
		border-radius: var(--radius-lg);
	}

	.button:hover:not(:disabled) {
		opacity: var(--opacity-hover);
	}

	.button:active:not(:disabled) {
		transform: scale(var(--motion-press-button));
	}

	.button:disabled {
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	/* Busy is not disabled: full emphasis, no pointer invitation. */
	.button.loading:disabled {
		opacity: 1;
		cursor: default;
	}

	.button.loading .label {
		visibility: hidden;
	}

	.spinner {
		position: absolute;
		width: 1.15em;
		height: 1.15em;
		border: var(--border-emphasis) solid currentColor;
		border-top-color: transparent;
		border-radius: var(--radius-full);
		/* One revolution, slower than any transition in the system: this is a
		   wait, not a state change (the desktop spinner's 800ms, same reason). */
		animation: spin 800ms linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(1turn);
		}
	}

	@keyframes pulse {
		50% {
			opacity: 1;
		}
	}

	/* Still an answer to "is anything happening", without the rotation. */
	@media (prefers-reduced-motion: reduce) {
		.spinner {
			border-top-color: currentColor;
			opacity: var(--opacity-dim);
			animation: pulse 1.2s ease-in-out infinite;
		}
	}

	.primary {
		background: var(--color-accent-base);
		color: var(--color-onAccent);
	}

	.secondary {
		background: transparent;
		border: var(--border-hairline) solid var(--color-border-strong);
		color: var(--color-fg-muted);
	}

	.danger {
		background: var(--color-error-base);
		color: var(--color-onAccent);
	}
</style>
