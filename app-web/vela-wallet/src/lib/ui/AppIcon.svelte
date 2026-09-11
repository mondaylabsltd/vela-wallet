<script lang="ts">
	/**
	 * The application icon — THE canonical mark (docs/design/icon/app-icon.svg),
	 * drawn inline for the one surface that leaves the app: the receive share
	 * card. Geometry and colours are the icon file's, verbatim (asset content,
	 * audit-whitelisted like `BrandMark.svelte`), because a picture in someone
	 * else's chat has no theme to follow and must look like the icon on their
	 * home screen.
	 */
	import { APP_ICON } from './brand-mark';

	interface Props {
		/** Rendered box in px. */
		size?: number;
	}

	let { size = 56 }: Props = $props();
</script>

<svg viewBox={APP_ICON.viewBox} width={size} height={size} aria-hidden="true">
	<rect
		x={APP_ICON.plate.x}
		y={APP_ICON.plate.y}
		width={APP_ICON.plate.size}
		height={APP_ICON.plate.size}
		rx={APP_ICON.plate.rx}
		fill={APP_ICON.plate.fill}
	/>
	{#each APP_ICON.paths as path (path.d)}
		<path d={path.d} fill={path.fill} />
	{/each}
</svg>
