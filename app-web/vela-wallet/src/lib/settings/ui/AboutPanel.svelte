<script lang="ts">
	/**
	 * ST14 / DST8 — the mark, the name and the build, the links, and the one
	 * slogan at the foot (owner, 2026-10-29). The build is a button: a tap puts
	 * "Vela v0.9.8 (ced8298)" on the clipboard for a support request, and says
	 * so for a moment in a neutral word.
	 */
	import { onDestroy } from 'svelte';
	import type { AboutModel } from '../model';
	import BrandMark from '$lib/ui/BrandMark.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import KeyValueRow from './KeyValueRow.svelte';

	interface Props {
		panel: AboutModel;
		/** Desktop lays the mark beside the name; the phone stacks them. */
		layout?: 'stacked' | 'inline';
	}

	let { panel, layout = 'stacked' }: Props = $props();

	// The mark is drawn INTO the circle the mock shows, so it is sized to
	// fit it rather than to fill it. A prop, not CSS — BrandMark writes width
	// and height attributes on its own <svg>.
	const MARK_SIZE = 32;

	/** "Copied", for as long as KeysBlock's own copy buttons say it. */
	const COPIED_MS = 1200;
	let copied = $state(false);
	let copiedTimer: ReturnType<typeof setTimeout> | undefined;

	async function copyVersion() {
		try {
			await navigator.clipboard.writeText(panel.versionCopy);
			copied = true;
			clearTimeout(copiedTimer);
			copiedTimer = setTimeout(() => (copied = false), COPIED_MS);
		} catch {
			// No clipboard permission: the build is on screen and selectable.
		}
	}

	onDestroy(() => clearTimeout(copiedTimer));
</script>

<div class="hero {layout}">
	<span class="mark"><BrandMark size={MARK_SIZE} /></span>
	<div class="titles">
		<p class="name">{panel.name}</p>
		<button
			type="button"
			class="version"
			aria-label={`${panel.copyLabel} ${panel.versionCopy}`}
			onclick={copyVersion}
		>
			<span class="value">{panel.version}</span>
			<Icon icon={UTILITY_ICONS[copied ? 'check' : 'copy']} size="xs" />
		</button>
		<!-- Said, not only drawn: the word is the confirmation a reader hears. -->
		<span class="copied" role="status">{copied ? panel.copiedLabel : ''}</span>
	</div>
</div>

{#if panel.sectionLinks !== undefined}
	<p class="section">{panel.sectionLinks}</p>
{/if}
{#each panel.links as row (row.label)}
	<KeyValueRow {row} />
{/each}

<p class="footer">{panel.footer}</p>

<style>
	.hero {
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		padding-block: var(--space-3xl);
	}

	.hero.stacked {
		flex-direction: column;
		text-align: center;
	}

	.mark {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-emptyStateCircle);
		height: var(--size-emptyStateCircle);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		flex-shrink: 0;
	}

	.titles {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.stacked .titles {
		align-items: center;
	}

	.inline .titles {
		align-items: flex-start;
	}

	.name {
		margin: 0;
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.version {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-xs) var(--space-sm);
		margin-inline: calc(var(--space-sm) * -1);
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.version:hover {
		color: var(--color-fg-muted);
	}

	/* The confirmation keeps its line, so nothing under it moves when it shows. */
	.copied {
		min-height: calc(var(--text-sm) * var(--text-scale, 1) * var(--leading-normal));
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.section {
		margin: var(--space-3xl) 0 var(--space-md);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.footer {
		margin: var(--space-3xl) 0 0;
		text-align: center;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
