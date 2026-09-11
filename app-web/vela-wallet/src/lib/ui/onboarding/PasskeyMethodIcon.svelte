<script lang="ts">
	/**
	 * One passkey method icon, inline (spec 038, issue 190).
	 *
	 * Two kinds: a lucide glyph from the wallet's own corpus (the device, the
	 * scanner) drawn by the shared `Icon`, or a filled mark from the passkey
	 * contract (the USB key), whose `ink` is `currentColor` and whose `muted`
	 * and `paper` roles keep the key's slots readable over the mark.
	 */
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import { PASSKEY_ICONS, type MethodGlyph } from '$lib/onboarding/passkey-icons';

	let { glyph }: { glyph: MethodGlyph } = $props();

	const mark = $derived(glyph.kind === 'mark' ? PASSKEY_ICONS[glyph.id] : undefined);
	const scale = $derived(mark ? 16 / mark.viewBox[0] : 1);
	const paint = (role: 'ink' | 'muted' | 'paper') =>
		role === 'ink'
			? 'currentColor'
			: role === 'muted'
				? 'var(--color-fg-subtle)'
				: 'var(--color-bg-base)';
</script>

<!-- Decoration: the row's name says which method this is. -->
{#if glyph.kind === 'lucide'}
	<span class="icon lucide" aria-hidden="true"
		><Icon icon={UTILITY_ICONS[glyph.name]} size="md" /></span
	>
{:else if mark}
	<svg class="icon" viewBox="0 0 16 16" role="presentation" aria-hidden="true">
		<g transform={scale === 1 ? undefined : `scale(${scale})`}>
			{#each mark.elements as el, i (i)}
				{#if el.tag === 'path'}
					<path
						d={el.attrs.d}
						fill={paint(el.role)}
						fill-rule={el.attrs['fill-rule'] as 'evenodd' | 'nonzero' | undefined}
						clip-rule={el.attrs['clip-rule'] as 'evenodd' | 'nonzero' | undefined}
					/>
				{:else if el.tag === 'rect'}
					<rect
						x={el.attrs.x}
						y={el.attrs.y}
						width={el.attrs.width}
						height={el.attrs.height}
						fill={paint(el.role)}
					/>
				{:else}
					<circle cx={el.attrs.cx} cy={el.attrs.cy} r={el.attrs.r} fill={paint(el.role)} />
				{/if}
			{/each}
		</g>
	</svg>
{/if}

<style>
	.icon {
		flex: 0 0 var(--icon-lg);
		width: var(--icon-lg);
		height: var(--icon-lg);
		color: var(--color-fg-muted);
	}
	.lucide {
		display: inline-flex;
		align-items: center;
		justify-content: center;
	}
</style>
