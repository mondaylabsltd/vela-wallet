<script lang="ts">
	/**
	 * One passkey method icon, inline (spec 038, issue 190).
	 *
	 * `ink` is `currentColor` so the row's own colour tints it; `muted` and
	 * `paper` are the two tokens the USB key needs to keep its slots readable
	 * over the mark. The 80-unit USB drawing is scaled into the 16-unit box the
	 * other five share, so every row's icon has one optical size.
	 */
	import { PASSKEY_ICONS, type PasskeyIconId } from '$lib/onboarding/passkey-icons';

	let { id }: { id: PasskeyIconId } = $props();

	const icon = $derived(PASSKEY_ICONS[id]);
	const scale = $derived(16 / icon.viewBox[0]);
	const paint = (role: 'ink' | 'muted' | 'paper') =>
		role === 'ink'
			? 'currentColor'
			: role === 'muted'
				? 'var(--color-fg-subtle)'
				: 'var(--color-bg-base)';
</script>

<!-- Decoration: the row's name says which method this is. -->
<svg class="icon" viewBox="0 0 16 16" role="presentation" aria-hidden="true">
	<g transform={scale === 1 ? undefined : `scale(${scale})`}>
		{#each icon.elements as el, i (i)}
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

<style>
	.icon {
		flex: 0 0 var(--icon-lg);
		width: var(--icon-lg);
		height: var(--icon-lg);
		color: var(--color-fg-muted);
	}
</style>
