<script lang="ts">
	/**
	 * ST14 / DST8 — the wordmark, the tagline, the build, the technical
	 * inventory and the three outbound links. Every value here is a fact about
	 * the running app, which is why the whole panel is key/value rows.
	 */
	import type { AboutModel, SwitchRowModel } from '../model';
	import BrandMark from '$lib/ui/BrandMark.svelte';
	import KeyValueRow from './KeyValueRow.svelte';
	import SwitchRow from './SwitchRow.svelte';

	interface Props {
		panel: AboutModel;
		/** Desktop lays the mark beside the tagline; the phone stacks them. */
		layout?: 'stacked' | 'inline';
		/**
		 * "Share anonymous usage statistics" — the web wallet's and the
		 * extension's, under the privacy policy that describes it. Absent in
		 * the gallery.
		 */
		analytics?: SwitchRowModel;
		onanalytics?: (on: boolean) => void;
	}

	let { panel, layout = 'stacked', analytics, onanalytics }: Props = $props();

	// The mark is drawn INTO the circle the mock shows, so it is sized to
	// fit it rather than to fill it. A prop, not CSS — BrandMark writes width
	// and height attributes on its own <svg>.
	const MARK_SIZE = 32;
</script>

<div class="hero {layout}">
	<span class="mark"><BrandMark size={MARK_SIZE} /></span>
	<div class="titles">
		<p class="tagline">{panel.tagline}</p>
		<p class="version">{panel.version}</p>
	</div>
</div>

<p class="section">{panel.sectionTechnical}</p>
{#each panel.rows as row (row.label)}
	<KeyValueRow {row} />
{/each}

{#if panel.sectionLinks !== undefined}
	<p class="section">{panel.sectionLinks}</p>
{/if}
{#each panel.links as row (row.label)}
	<KeyValueRow {row} />
{/each}

{#if analytics !== undefined}
	<div class="privacy"><SwitchRow row={analytics} onchange={onanalytics} /></div>
{/if}

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
		gap: var(--space-sm);
	}

	.tagline {
		margin: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.version {
		margin: 0;
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.section {
		margin: var(--space-3xl) 0 var(--space-md);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	/* Its own group under the links: a hairline above, as between rows. */
	.privacy {
		margin-top: var(--space-xl);
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.footer {
		margin: var(--space-3xl) 0 0;
		text-align: center;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
