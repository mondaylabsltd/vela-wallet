/**
 * The brand mark as data (docs/design-system.md §Brand) — the same three paths
 * `BrandMark.svelte` draws, for surfaces that cannot mount a component: the
 * receive share image is composed as an SVG string and rasterised, so it
 * needs the geometry and the dark-UI asset's own colours as values. Asset
 * content, audit-whitelisted like the component is.
 */
export const BRAND_MARK = {
	viewBox: '0 0 258 260',
	width: 258,
	height: 260,
	paths: [
		{ d: 'M122,0C70,53,38,118,18,187L122,187L122,0Z', fill: '#ff6a45' },
		{ d: 'M142,42C193,75,225,128,240,187L142,187L142,42Z', fill: '#ffa98e' },
		{ d: 'M0,207L258,207C243,240,211,260,165,260L92,260C49,260,16,240,0,207Z', fill: '#ded5ce' }
	]
} as const;

/**
 * The application icon as data — `docs/design/icon/app-icon.svg`, THE canonical
 * mark every platform's icon is rendered from, verbatim. The receive share
 * card wears this rather than the in-app sailboat (founder, 2026-09-05): the
 * saved picture ends up beside the app on someone's phone, and the two must
 * be the same mark. Consumed by `AppIcon.svelte` and `share-image.ts`.
 */
export const APP_ICON = {
	viewBox: '0 0 68 68',
	plate: { x: 1, y: 1, size: 66, rx: 18, fill: '#f46d50' },
	paths: [
		{ d: 'M33,12C25,21,20,32,17,42L33,42L33,12Z', fill: '#fff3ec' },
		{ d: 'M36,19C45,25,50,34,52,42L36,42L36,19Z', fill: '#ffc6b0' },
		{ d: 'M13,46L55,46C52,52,47,55,40,55L28,55C21,55,16,52,13,46Z', fill: '#5a4037' }
	]
} as const;
