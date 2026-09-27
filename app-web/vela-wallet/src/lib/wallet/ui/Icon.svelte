<script lang="ts">
	import type { IconDef } from '../icons';

	interface Props {
		icon: IconDef;
		/** Token icon size step. */
		size?: 'xs' | 'sm' | 'base' | 'md' | 'lg' | 'xl' | 'tab';
		/** Accessible name; decorative (hidden) when omitted. */
		label?: string;
	}

	let { icon, size = 'base', label }: Props = $props();
</script>

<svg
	class="icon {size}"
	viewBox="0 0 24 24"
	role={label === undefined ? 'presentation' : 'img'}
	aria-label={label}
	aria-hidden={label === undefined ? 'true' : undefined}
	fill={icon.style === 'fill' ? 'currentColor' : 'none'}
	stroke={icon.style === 'stroke' ? 'currentColor' : 'none'}
	stroke-linecap="round"
	stroke-linejoin="round"
>
	{#if icon.style === 'fill'}
		{#each icon.paths as d (d)}<path {d} />{/each}
	{:else if icon.style === 'mixed'}
		{#each icon.elements as el, i (i)}
			{@const fill = el.mode === 'fill' ? 'currentColor' : 'none'}
			{@const stroke = el.mode === 'stroke' ? 'currentColor' : 'none'}
			{#if el.tag === 'path'}
				<path d={el.d} {fill} {stroke} fill-rule={el.fillRule} />
			{:else if el.tag === 'circle'}
				<circle cx={el.cx} cy={el.cy} r={el.r} {fill} {stroke} />
			{:else if el.tag === 'rect'}
				<rect width={el.width} height={el.height} x={el.x} y={el.y} rx={el.rx} {fill} {stroke} />
			{:else if el.tag === 'line'}
				<line x1={el.x1} x2={el.x2} y1={el.y1} y2={el.y2} {fill} {stroke} />
			{:else}
				<polyline points={el.points} {fill} {stroke} />
			{/if}
		{/each}
	{:else}
		{#each icon.elements as el, i (i)}
			{#if el.tag === 'path'}
				<path d={el.d} />
			{:else if el.tag === 'circle'}
				<circle cx={el.cx} cy={el.cy} r={el.r} />
			{:else if el.tag === 'rect'}
				<rect width={el.width} height={el.height} x={el.x} y={el.y} rx={el.rx} />
			{:else if el.tag === 'line'}
				<line x1={el.x1} x2={el.x2} y1={el.y1} y2={el.y2} />
			{:else}
				<polyline points={el.points} />
			{/if}
		{/each}
	{/if}
</svg>

<style>
	.icon {
		display: block;
		flex-shrink: 0;
		stroke-width: var(--icon-stroke-base);
	}

	.xs {
		width: var(--icon-xs);
		height: var(--icon-xs);
	}

	.sm {
		width: var(--icon-sm);
		height: var(--icon-sm);
	}

	.base {
		width: var(--icon-base);
		height: var(--icon-base);
	}

	.md {
		width: var(--icon-md);
		height: var(--icon-md);
	}

	.lg {
		width: var(--icon-lg);
		height: var(--icon-lg);
	}

	.xl {
		width: var(--icon-xl);
		height: var(--icon-xl);
	}

	/* The phone tab bar's glyph, which carries the tab on its own. */
	.tab {
		width: var(--icon-tab);
		height: var(--icon-tab);
	}
</style>
