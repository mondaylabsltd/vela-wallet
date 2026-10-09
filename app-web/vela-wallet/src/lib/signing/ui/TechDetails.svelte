<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import LetterAvatar from '$lib/ui/LetterAvatar.svelte';
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import KeyValueRows from './KeyValueRows.svelte';
	import type { TechModel } from '../model';

	/**
	 * The universal fallback renderer (SPEC 签名 · 技术细节): function → params
	 * → address identities → simulation → raw data, five fixed layers that can
	 * render ANY request, folded away by default and never removed. Whatever
	 * the descriptor could not explain is still in here, in full.
	 */
	interface Props {
		tech: TechModel;
		open: boolean;
		ontoggle?: () => void;
	}

	let { tech, open, ontoggle }: Props = $props();
</script>

<section class="tech" class:open>
	<button type="button" class="toggle" aria-expanded={open} onclick={ontoggle}>
		<Icon icon={UTILITY_ICONS[open ? 'chevron-down' : 'chevron-right']} size="sm" />
		<span>{tech.title}{tech.summary ? ` · ${tech.summary}` : ''}</span>
	</button>

	{#if open}
		<div class="panel">
			{#if tech.fn}
				<p class="layer-label">{tech.fn.label}</p>
				<p class="signature">{tech.fn.signature}</p>
			{/if}

			{#if tech.params.length > 0}
				<KeyValueRows rows={tech.params} />
			{/if}

			{#each tech.identities as identity, i (i)}
				<div class="identity">
					{#if identity.mark}
						<TokenIcon
							ticker={identity.mark.ticker}
							badgeColor={identity.mark.badgeColor}
							logoUrls={identity.mark.logoUrls}
							badgeLogoUrl={identity.mark.badgeLogoUrl}
							badgeHidden={identity.mark.badgeHidden}
							size="inline"
						/>
					{:else if identity.avatar}
						<LetterAvatar letter={identity.avatar.letter} tint={identity.avatar.tint} size={24} />
					{/if}
					<span class="identity-text">
						<span class="role">{identity.role} · {identity.name}</span>
						<span class="address">{identity.address}</span>
					</span>
					<span class="tools">
						<Icon icon={UTILITY_ICONS.copy} size="sm" label={tech.copyLabel} />
						<Icon icon={UTILITY_ICONS['external-link']} size="sm" label={tech.explorerLabel} />
					</span>
				</div>
			{/each}

			{#if tech.simResult}
				<KeyValueRows rows={[tech.simResult]} />
			{/if}

			{#if tech.raw}
				<p class="layer-label">{tech.raw.label}</p>
				<p class="raw">{tech.raw.hex}</p>
			{/if}
		</div>
	{/if}
</section>

<style>
	.tech {
		border-radius: var(--radius-xl);
	}

	.open {
		padding: var(--space-lg) var(--space-xl);
		background: var(--color-bg-sunken);
	}

	.toggle {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		padding: var(--space-lg) 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
		text-align: start;
	}

	.panel {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		padding-bottom: var(--space-lg);
	}

	.layer-label {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.signature,
	.raw {
		margin: 0;
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-relaxed);
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.raw {
		color: var(--color-fg-muted);
	}

	.identity {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding-block: var(--space-md);
	}

	.identity-text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.role {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.tools {
		display: flex;
		gap: var(--space-md);
		color: var(--color-fg-muted);
	}
</style>
