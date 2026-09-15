<script lang="ts">
	import { page } from '$app/state';
	import { LOCALES, SUPPORTED_LOCALES, switchTo, type Locale } from '$lib/i18n/locales';
	import { catalog } from '$lib/i18n/resolve';

	/**
	 * The language switcher (spec 059, FR-012).
	 *
	 * Two rules it exists to keep:
	 *
	 *  1. **It lands on the same page.** Sending a reader to the home page because
	 *     they wanted this page in their language is the single most common i18n
	 *     failure, and `switchTo` is what prevents it (SC-003).
	 *  2. **Every language is named in itself.** 日本語, not "Japanese" — the row
	 *     has to be readable by somebody who cannot read the current UI, which is
	 *     the whole reason they are looking at this menu.
	 *
	 * It is a plain `<details>` so it works with JavaScript disabled, like the
	 * rest of a prerendered page.
	 */
	let { locale }: { locale: Locale } = $props();

	const m = $derived(catalog(locale));
	const pathname = $derived(page.url.pathname);
	let open = $state(false);
</script>

<details class="switcher" bind:open>
	<summary aria-label={m.chrome.language.choose}>
		<svg
			width="15"
			height="15"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="1.8"
			aria-hidden="true"
		>
			<circle cx="12" cy="12" r="9" />
			<path d="M3 12h18M12 3a15 15 0 010 18a15 15 0 010-18" stroke-linecap="round" />
		</svg>
		<span class="current">{LOCALES[locale].endonym}</span>
	</summary>
	<ul>
		{#each SUPPORTED_LOCALES as tag (tag)}
			<li>
				<a
					href={switchTo(tag, pathname)}
					hreflang={tag}
					lang={tag}
					aria-current={tag === locale ? 'true' : undefined}
					class:active={tag === locale}
					onclick={() => (open = false)}
					data-rybbit-event="language_switch"
					data-rybbit-prop-locale={tag}
				>
					{LOCALES[tag].endonym}
				</a>
			</li>
		{/each}
	</ul>
</details>

<style>
	.switcher {
		position: relative;
	}
	summary {
		display: flex;
		align-items: center;
		gap: 6px;
		cursor: pointer;
		list-style: none;
		padding: 5px 9px;
		border-radius: 8px;
		color: var(--text-secondary);
		font-size: 0.82rem;
		font-weight: 500;
		white-space: nowrap;
		transition:
			color 0.15s,
			background 0.15s;
	}
	summary::-webkit-details-marker {
		display: none;
	}
	summary:hover {
		color: var(--text);
		background: var(--bg-sunken);
	}
	ul {
		position: absolute;
		right: 0;
		top: calc(100% + 6px);
		z-index: 60;
		margin: 0;
		padding: 6px;
		list-style: none;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: 12px;
		box-shadow: var(--shadow-md);
		min-width: 190px;
		max-height: 60vh;
		overflow-y: auto;
	}
	li a {
		display: block;
		padding: 7px 12px;
		border-radius: 8px;
		font-size: 0.85rem;
		color: var(--text-secondary);
		text-decoration: none;
		white-space: nowrap;
	}
	li a:hover {
		background: var(--bg-sunken);
		color: var(--text);
	}
	li a.active {
		color: var(--accent);
		font-weight: 600;
	}

	/* On a narrow screen the current-language label is the first thing to go —
	   the globe still says what the control is. */
	@media (max-width: 520px) {
		.current {
			display: none;
		}
	}
</style>
