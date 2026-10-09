/**
 * Wallet icon corpus — the web port of
 * `specs/015-wallet-home-ui/contracts/icons.json` (research.md D2).
 *
 * All glyphs are lucide v1.11.0 (ISC), 24×24, currentColor. Nav outline =
 * verbatim lucide stroke defs; nav solid = fills derived from the same
 * geometry (closed subpaths filled, evenodd holes; the users back-person arcs
 * stay stroked), so selection swaps style without the tab shifting.
 */

export type IconElement =
	| { tag: 'path'; d: string }
	| { tag: 'circle'; cx: string; cy: string; r: string }
	| { tag: 'rect'; width: string; height: string; x: string; y: string; rx: string }
	| { tag: 'polyline'; points: string }
	| { tag: 'line'; x1: string; x2: string; y1: string; y2: string };

export type MixedElement = IconElement & { mode: 'fill' | 'stroke'; fillRule?: 'evenodd' };

export type IconDef =
	| { style: 'stroke'; elements: IconElement[] }
	| { style: 'fill'; paths: string[] }
	| { style: 'mixed'; elements: MixedElement[] };

export type NavIconId = 'wallet' | 'contacts' | 'explore' | 'settings';

export const NAV_ICONS: Record<NavIconId, { outline: IconDef; solid: IconDef }> = {
	wallet: {
		outline: {
			style: 'stroke',
			elements: [
				{
					tag: 'path',
					d: 'M19 7V4a1 1 0 0 0-1-1H5a2 2 0 0 0 0 4h15a1 1 0 0 1 1 1v4h-3a2 2 0 0 0 0 4h3a1 1 0 0 0 1-1v-2a1 1 0 0 0-1-1'
				},
				{ tag: 'path', d: 'M3 5v14a2 2 0 0 0 2 2h15a1 1 0 0 0 1-1v-4' }
			]
		},
		solid: {
			style: 'mixed',
			elements: [
				{
					tag: 'path',
					mode: 'fill',
					d: 'M18 3a1 1 0 0 1 1 1v3h1a1 1 0 0 1 1 1v3h-4a2 2 0 0 0 0 4h4v4a1 1 0 0 1-1 1H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h13z'
				}
			]
		}
	},
	contacts: {
		outline: {
			style: 'stroke',
			elements: [
				{ tag: 'path', d: 'M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2' },
				{ tag: 'path', d: 'M16 3.128a4 4 0 0 1 0 7.744' },
				{ tag: 'path', d: 'M22 21v-2a4 4 0 0 0-3-3.87' },
				{ tag: 'circle', cx: '9', cy: '7', r: '4' }
			]
		},
		solid: {
			style: 'mixed',
			elements: [
				{ tag: 'path', mode: 'fill', d: 'M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2z' },
				{ tag: 'circle', mode: 'fill', cx: '9', cy: '7', r: '4' },
				{ tag: 'path', mode: 'stroke', d: 'M16 3.128a4 4 0 0 1 0 7.744' },
				{ tag: 'path', mode: 'stroke', d: 'M22 21v-2a4 4 0 0 0-3-3.87' }
			]
		}
	},
	explore: {
		outline: {
			style: 'stroke',
			elements: [
				{ tag: 'circle', cx: '12', cy: '12', r: '10' },
				{
					tag: 'path',
					d: 'm16.24 7.76-1.804 5.411a2 2 0 0 1-1.265 1.265L7.76 16.24l1.804-5.411a2 2 0 0 1 1.265-1.265z'
				}
			]
		},
		solid: {
			style: 'mixed',
			elements: [
				{
					tag: 'path',
					mode: 'fill',
					fillRule: 'evenodd',
					d: 'M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zM16.24 7.76l-1.804 5.411a2 2 0 0 1-1.265 1.265L7.76 16.24l1.804-5.411a2 2 0 0 1 1.265-1.265z'
				}
			]
		}
	},
	settings: {
		outline: {
			style: 'stroke',
			elements: [
				{
					tag: 'path',
					d: 'M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915'
				},
				{ tag: 'circle', cx: '12', cy: '12', r: '3' }
			]
		},
		solid: {
			style: 'mixed',
			elements: [
				{
					tag: 'path',
					mode: 'fill',
					fillRule: 'evenodd',
					d: 'M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z'
				}
			]
		}
	}
};

export type UtilityIconId =
	| 'arrow-down-left'
	| 'arrow-up-right'
	| 'scan-line'
	| 'eye'
	| 'eye-off'
	| 'search'
	| 'x'
	| 'copy'
	| 'chevron-right'
	| 'chevron-down'
	| 'link-2'
	| 'triangle-alert'
	| 'refresh-cw'
	| 'check'
	| 'inbox'
	| 'wallet'
	// spec 018 additions (specs/018-contacts-ui/contracts/icons.json)
	| 'user-round-plus'
	| 'users-round'
	| 'folder-plus'
	| 'download'
	| 'upload'
	| 'pencil'
	| 'trash-2'
	| 'ellipsis'
	| 'qr-code'
	| 'plus'
	| 'chevron-left'
	// spec 021 additions (specs/021-wallet-flows-ui/contracts/icons.json)
	| 'user-round'
	| 'chevrons-up-down'
	| 'credit-card'
	| 'clock'
	| 'file-text'
	| 'image'
	| 'zap'
	| 'rotate-ccw'
	// spec 022 additions (explore browser chrome + signing sheet). `star` is a
	// computed five-point path, not a remembered lucide one — a mis-recalled
	// star draws a shape nobody can name.
	| 'arrow-left'
	| 'arrow-right'
	| 'arrow-down'
	| 'star'
	| 'star-filled'
	| 'share-2'
	| 'power'
	| 'lock'
	| 'external-link'
	| 'compass'
	// spec 023 additions (the settings rows' leading glyphs + their chrome)
	| 'globe'
	| 'sun'
	| 'moon'
	| 'monitor'
	| 'laptop'
	| 'smartphone'
	| 'coins'
	| 'hash'
	| 'calendar'
	| 'network'
	| 'server'
	| 'hard-drive'
	| 'info'
	| 'log-out'
	| 'message-square-text'
	| 'circle-alert'
	// issue 460: a failure's mark — the ✕ is the sheet's close, never a status
	| 'exclamation'
	// 078 round 3: the bug report's screenshots
	| 'image-plus'
	// 078: Settings → Community — its nav glyph, and the three brands' marks
	| 'messages-square'
	| 'brand-x'
	| 'brand-telegram'
	| 'brand-discord'
	// spec 102: what is trusted — a signing page you trust, Settings → Signing pages
	| 'shield-check';

export const UTILITY_ICONS: Record<UtilityIconId, IconDef> = {
	'arrow-down-left': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M17 7 7 17' },
			{ tag: 'path', d: 'M17 17H7V7' }
		]
	},
	'arrow-up-right': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M7 7h10v10' },
			{ tag: 'path', d: 'M7 17 17 7' }
		]
	},
	'scan-line': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M3 7V5a2 2 0 0 1 2-2h2' },
			{ tag: 'path', d: 'M17 3h2a2 2 0 0 1 2 2v2' },
			{ tag: 'path', d: 'M21 17v2a2 2 0 0 1-2 2h-2' },
			{ tag: 'path', d: 'M7 21H5a2 2 0 0 1-2-2v-2' },
			{ tag: 'path', d: 'M7 12h10' }
		]
	},
	eye: {
		style: 'stroke',
		elements: [
			{
				tag: 'path',
				d: 'M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0'
			},
			{ tag: 'circle', cx: '12', cy: '12', r: '3' }
		]
	},
	'eye-off': {
		style: 'stroke',
		elements: [
			{
				tag: 'path',
				d: 'M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49'
			},
			{ tag: 'path', d: 'M14.084 14.158a3 3 0 0 1-4.242-4.242' },
			{
				tag: 'path',
				d: 'M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143'
			},
			{ tag: 'path', d: 'm2 2 20 20' }
		]
	},
	search: {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'm21 21-4.34-4.34' },
			{ tag: 'circle', cx: '11', cy: '11', r: '8' }
		]
	},
	x: {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M18 6 6 18' },
			{ tag: 'path', d: 'm6 6 12 12' }
		]
	},
	copy: {
		style: 'stroke',
		elements: [
			{ tag: 'rect', width: '14', height: '14', x: '8', y: '8', rx: '2' },
			{ tag: 'path', d: 'M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2' }
		]
	},
	'chevron-right': { style: 'stroke', elements: [{ tag: 'path', d: 'm9 18 6-6-6-6' }] },
	'chevron-down': { style: 'stroke', elements: [{ tag: 'path', d: 'm6 9 6 6 6-6' }] },
	'link-2': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M9 17H7A5 5 0 0 1 7 7h2' },
			{ tag: 'path', d: 'M15 7h2a5 5 0 1 1 0 10h-2' },
			{ tag: 'line', x1: '8', x2: '16', y1: '12', y2: '12' }
		]
	},
	'triangle-alert': {
		style: 'stroke',
		elements: [
			{
				tag: 'path',
				d: 'm21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3'
			},
			{ tag: 'path', d: 'M12 9v4' },
			{ tag: 'path', d: 'M12 17h.01' }
		]
	},
	'refresh-cw': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8' },
			{ tag: 'path', d: 'M21 3v5h-5' },
			{ tag: 'path', d: 'M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16' },
			{ tag: 'path', d: 'M8 16H3v5' }
		]
	},
	check: { style: 'stroke', elements: [{ tag: 'path', d: 'M20 6 9 17l-5-5' }] },
	inbox: {
		style: 'stroke',
		elements: [
			{ tag: 'polyline', points: '22 12 16 12 14 15 10 15 8 12 2 12' },
			{
				tag: 'path',
				d: 'M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z'
			}
		]
	},
	wallet: {
		style: 'stroke',
		elements: [
			{
				tag: 'path',
				d: 'M19 7V4a1 1 0 0 0-1-1H5a2 2 0 0 0 0 4h15a1 1 0 0 1 1 1v4h-3a2 2 0 0 0 0 4h3a1 1 0 0 0 1-1v-2a1 1 0 0 0-1-1'
			},
			{ tag: 'path', d: 'M3 5v14a2 2 0 0 0 2 2h15a1 1 0 0 0 1-1v-4' }
		]
	},
	'user-round-plus': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M2 21a8 8 0 0 1 13.292-6' },
			{ tag: 'circle', cx: '10', cy: '8', r: '5' },
			{ tag: 'path', d: 'M19 16v6' },
			{ tag: 'path', d: 'M22 19h-6' }
		]
	},
	'users-round': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M18 21a8 8 0 0 0-16 0' },
			{ tag: 'circle', cx: '10', cy: '8', r: '5' },
			{ tag: 'path', d: 'M22 20c0-3.37-2-6.5-4-8a5 5 0 0 0-.45-8.3' }
		]
	},
	'folder-plus': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M12 10v6' },
			{ tag: 'path', d: 'M9 13h6' },
			{
				tag: 'path',
				d: 'M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z'
			}
		]
	},
	download: {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M12 15V3' },
			{ tag: 'path', d: 'M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4' },
			{ tag: 'path', d: 'm7 10 5 5 5-5' }
		]
	},
	upload: {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M12 3v12' },
			{ tag: 'path', d: 'm17 8-5-5-5 5' },
			{ tag: 'path', d: 'M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4' }
		]
	},
	pencil: {
		style: 'stroke',
		elements: [
			{
				tag: 'path',
				d: 'M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z'
			},
			{ tag: 'path', d: 'm15 5 4 4' }
		]
	},
	'trash-2': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M10 11v6' },
			{ tag: 'path', d: 'M14 11v6' },
			{ tag: 'path', d: 'M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6' },
			{ tag: 'path', d: 'M3 6h18' },
			{ tag: 'path', d: 'M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2' }
		]
	},
	ellipsis: {
		style: 'stroke',
		elements: [
			{ tag: 'circle', cx: '12', cy: '12', r: '1' },
			{ tag: 'circle', cx: '19', cy: '12', r: '1' },
			{ tag: 'circle', cx: '5', cy: '12', r: '1' }
		]
	},
	'qr-code': {
		style: 'stroke',
		elements: [
			{ tag: 'rect', width: '5', height: '5', x: '3', y: '3', rx: '1' },
			{ tag: 'rect', width: '5', height: '5', x: '16', y: '3', rx: '1' },
			{ tag: 'rect', width: '5', height: '5', x: '3', y: '16', rx: '1' },
			{ tag: 'path', d: 'M21 16h-3a2 2 0 0 0-2 2v3' },
			{ tag: 'path', d: 'M21 21v.01' },
			{ tag: 'path', d: 'M12 7v3a2 2 0 0 1-2 2H7' },
			{ tag: 'path', d: 'M3 12h.01' },
			{ tag: 'path', d: 'M12 3h.01' },
			{ tag: 'path', d: 'M12 16v.01' },
			{ tag: 'path', d: 'M16 12h1' },
			{ tag: 'path', d: 'M21 12v.01' },
			{ tag: 'path', d: 'M12 21v-1' }
		]
	},
	plus: {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M5 12h14' },
			{ tag: 'path', d: 'M12 5v14' }
		]
	},
	'chevron-left': { style: 'stroke', elements: [{ tag: 'path', d: 'm15 18-6-6 6-6' }] },
	// The single-person glyph. `users-round` (spec 018) is the group; SD2's
	// recipient field opens a picker for exactly one person, and drawing two
	// heads there reads as "add several".
	'user-round': {
		style: 'stroke',
		elements: [
			{ tag: 'circle', cx: '12', cy: '8', r: '5' },
			{ tag: 'path', d: 'M20 21a8 8 0 0 0-16 0' }
		]
	},
	// SD2's denomination toggle: the amount is enterable in the token or in
	// the display currency, and this is the affordance that says so.
	'chevrons-up-down': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'm7 15 5 5 5-5' },
			{ tag: 'path', d: 'm7 9 5-5 5 5' }
		]
	},
	'credit-card': {
		style: 'stroke',
		elements: [
			{ tag: 'rect', width: '20', height: '14', x: '2', y: '5', rx: '2' },
			{ tag: 'line', x1: '2', x2: '22', y1: '10', y2: '10' }
		]
	},
	clock: {
		style: 'stroke',
		elements: [
			{ tag: 'circle', cx: '12', cy: '12', r: '10' },
			{ tag: 'polyline', points: '12 6 12 12 16 14' }
		]
	},
	'file-text': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z' },
			{ tag: 'path', d: 'M14 2v4a2 2 0 0 0 2 2h4' },
			{ tag: 'path', d: 'M10 9H8' },
			{ tag: 'path', d: 'M16 13H8' },
			{ tag: 'path', d: 'M16 17H8' }
		]
	},
	image: {
		style: 'stroke',
		elements: [
			{ tag: 'rect', width: '18', height: '18', x: '3', y: '3', rx: '2' },
			{ tag: 'circle', cx: '9', cy: '9', r: '2' },
			{ tag: 'path', d: 'm21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21' }
		]
	},
	// Issue 460: a failure wears '!', not the ✕ — the ✕ is the sheet's close,
	// and a red one in a status disc reads as a dead close button. The same
	// two strokes as Android VelaIcons.Exclamation, iOS LucideGlyph.exclamation
	// and desktop Icon::Exclamation.
	exclamation: {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M12 6v7' },
			{ tag: 'path', d: 'M12 17h.01' }
		]
	},
	// lucide `image-plus`: the frame opened at its top-right for the plus.
	'image-plus': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M16 5h6' },
			{ tag: 'path', d: 'M19 2v6' },
			{ tag: 'path', d: 'M21 11.5V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7.5' },
			{ tag: 'path', d: 'm21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21' },
			{ tag: 'circle', cx: '9', cy: '9', r: '2' }
		]
	},
	// lucide `messages-square`: two speech bubbles — the Community destination.
	'messages-square': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M14 9a2 2 0 0 1-2 2H6l-4 4V4a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2z' },
			{ tag: 'path', d: 'M18 9h2a2 2 0 0 1 2 2v11l-4-4h-6a2 2 0 0 1-2-2v-1' }
		]
	},
	// The three brands' own marks, MONOCHROME (they take currentColor like
	// every glyph here, never a brand colour) — simple-icons paths, CC0 1.0.
	// Filled marks: a row draws them a step smaller than a stroked glyph so
	// the two weigh the same (SettingsRow `.brand`).
	'brand-x': {
		style: 'fill',
		paths: [
			'M18.901 1.153h3.68l-8.04 9.19L24 22.846h-7.406l-5.8-7.584-6.638 7.584H.474l8.6-9.83L0 1.154h7.594l5.243 6.932ZM17.61 20.644h2.039L6.486 3.24H4.298Z'
		]
	},
	'brand-telegram': {
		style: 'fill',
		paths: [
			'M11.944 0A12 12 0 0 0 0 12a12 12 0 0 0 12 12 12 12 0 0 0 12-12A12 12 0 0 0 12 0a12 12 0 0 0-.056 0zm4.962 7.224c.1-.002.321.023.465.14a.506.506 0 0 1 .171.325c.016.093.036.306.02.472-.18 1.898-.962 6.502-1.36 8.627-.168.9-.499 1.201-.82 1.23-.696.065-1.225-.46-1.9-.902-1.056-.693-1.653-1.124-2.678-1.8-1.185-.78-.417-1.21.258-1.91.177-.184 3.247-2.977 3.307-3.23.007-.032.014-.15-.056-.212s-.174-.041-.249-.024c-.106.024-1.793 1.14-5.061 3.345-.48.33-.913.49-1.302.48-.428-.008-1.252-.241-1.865-.44-.752-.245-1.349-.374-1.297-.789.027-.216.325-.437.893-.663 3.498-1.524 5.83-2.529 6.998-3.014 3.332-1.386 4.025-1.627 4.476-1.635z'
		]
	},
	'brand-discord': {
		style: 'fill',
		paths: [
			'M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286zM8.02 15.3312c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9555-2.4189 2.157-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.9555 2.4189-2.1569 2.4189zm7.9748 0c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9554-2.4189 2.1569-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.946 2.4189-2.1568 2.4189Z'
		]
	},
	zap: {
		style: 'stroke',
		elements: [
			{
				tag: 'path',
				d: 'M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z'
			}
		]
	},
	'rotate-ccw': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8' },
			{ tag: 'path', d: 'M3 3v5h5' }
		]
	},
	'arrow-left': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M19 12H5' },
			{ tag: 'path', d: 'm12 19-7-7 7-7' }
		]
	},
	'arrow-right': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M5 12h14' },
			{ tag: 'path', d: 'm12 5 7 7-7 7' }
		]
	},
	'arrow-down': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M12 5v14' },
			{ tag: 'path', d: 'm19 12-7 7-7-7' }
		]
	},
	star: {
		style: 'stroke',
		elements: [
			{
				tag: 'path',
				d: 'M12.00 2.70 L14.35 8.76 L20.84 9.13 L15.80 13.24 L17.47 19.52 L12.00 16.00 L6.53 19.52 L8.20 13.24 L3.16 9.13 L9.65 8.76 Z'
			}
		]
	},
	'star-filled': {
		style: 'fill',
		paths: [
			'M12.00 2.70 L14.35 8.76 L20.84 9.13 L15.80 13.24 L17.47 19.52 L12.00 16.00 L6.53 19.52 L8.20 13.24 L3.16 9.13 L9.65 8.76 Z'
		]
	},
	'share-2': {
		style: 'stroke',
		elements: [
			{ tag: 'circle', cx: '18', cy: '5', r: '3' },
			{ tag: 'circle', cx: '6', cy: '12', r: '3' },
			{ tag: 'circle', cx: '18', cy: '19', r: '3' },
			{ tag: 'line', x1: '8.59', x2: '15.42', y1: '13.51', y2: '17.49' },
			{ tag: 'line', x1: '15.41', x2: '8.59', y1: '6.51', y2: '10.49' }
		]
	},
	power: {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M12 2v10' },
			{ tag: 'path', d: 'M18.4 6.6a9 9 0 1 1-12.77.04' }
		]
	},
	lock: {
		style: 'stroke',
		elements: [
			{ tag: 'rect', width: '18', height: '11', x: '3', y: '11', rx: '2' },
			{ tag: 'path', d: 'M7 11V7a5 5 0 0 1 10 0v4' }
		]
	},
	'shield-check': {
		style: 'stroke',
		elements: [
			{
				tag: 'path',
				d: 'M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z'
			},
			{ tag: 'path', d: 'm9 12 2 2 4-4' }
		]
	},
	'external-link': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M15 3h6v6' },
			{ tag: 'path', d: 'M10 14 21 3' },
			{ tag: 'path', d: 'M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6' }
		]
	},
	compass: {
		style: 'stroke',
		elements: [
			{
				tag: 'path',
				d: 'm16.24 7.76-1.804 5.411a2 2 0 0 1-1.265 1.265L7.76 16.24l1.804-5.411a2 2 0 0 1 1.265-1.265z'
			},
			{ tag: 'circle', cx: '12', cy: '12', r: '10' }
		]
	},
	// spec 023 — lucide v1.11.0, verbatim stroke defs like every glyph above.
	globe: {
		style: 'stroke',
		elements: [
			{ tag: 'circle', cx: '12', cy: '12', r: '10' },
			{ tag: 'path', d: 'M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20' },
			{ tag: 'path', d: 'M2 12h20' }
		]
	},
	sun: {
		style: 'stroke',
		elements: [
			{ tag: 'circle', cx: '12', cy: '12', r: '4' },
			{ tag: 'path', d: 'M12 2v2' },
			{ tag: 'path', d: 'M12 20v2' },
			{ tag: 'path', d: 'm4.93 4.93 1.41 1.41' },
			{ tag: 'path', d: 'm17.66 17.66 1.41 1.41' },
			{ tag: 'path', d: 'M2 12h2' },
			{ tag: 'path', d: 'M20 12h2' },
			{ tag: 'path', d: 'm6.34 17.66-1.41 1.41' },
			{ tag: 'path', d: 'm19.07 4.93-1.41 1.41' }
		]
	},
	moon: {
		style: 'stroke',
		elements: [
			{
				tag: 'path',
				d: 'M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401'
			}
		]
	},
	// Spec 038 (#190, founder's revision): "this device" is the device, not a
	// vendor — a laptop where the pointer is fine, a phone where it is coarse.
	laptop: {
		style: 'stroke',
		elements: [
			{
				tag: 'path',
				d: 'M20 16V7a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v9m16 0H4m16 0 1.28 2.55a1 1 0 0 1-.9 1.45H3.62a1 1 0 0 1-.9-1.45L4 16'
			}
		]
	},
	smartphone: {
		style: 'stroke',
		elements: [
			{ tag: 'rect', width: '14', height: '20', x: '5', y: '2', rx: '2' },
			{ tag: 'path', d: 'M12 18h.01' }
		]
	},
	monitor: {
		style: 'stroke',
		elements: [
			{ tag: 'rect', width: '20', height: '14', x: '2', y: '3', rx: '2' },
			{ tag: 'line', x1: '8', x2: '16', y1: '21', y2: '21' },
			{ tag: 'line', x1: '12', x2: '12', y1: '17', y2: '21' }
		]
	},
	coins: {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M13.744 17.736a6 6 0 1 1-7.48-7.48' },
			{ tag: 'path', d: 'M15 6h1v4' },
			{ tag: 'path', d: 'm6.134 14.768.866-.5 2 3.464' },
			{ tag: 'circle', cx: '16', cy: '8', r: '6' }
		]
	},
	hash: {
		style: 'stroke',
		elements: [
			{ tag: 'line', x1: '4', x2: '20', y1: '9', y2: '9' },
			{ tag: 'line', x1: '4', x2: '20', y1: '15', y2: '15' },
			{ tag: 'line', x1: '10', x2: '8', y1: '3', y2: '21' },
			{ tag: 'line', x1: '16', x2: '14', y1: '3', y2: '21' }
		]
	},
	calendar: {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M8 2v4' },
			{ tag: 'path', d: 'M16 2v4' },
			{ tag: 'rect', width: '18', height: '18', x: '3', y: '4', rx: '2' },
			{ tag: 'path', d: 'M3 10h18' }
		]
	},
	network: {
		style: 'stroke',
		elements: [
			{ tag: 'rect', width: '6', height: '6', x: '16', y: '16', rx: '1' },
			{ tag: 'rect', width: '6', height: '6', x: '2', y: '16', rx: '1' },
			{ tag: 'rect', width: '6', height: '6', x: '9', y: '2', rx: '1' },
			{ tag: 'path', d: 'M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3' },
			{ tag: 'path', d: 'M12 12V8' }
		]
	},
	server: {
		style: 'stroke',
		elements: [
			{ tag: 'rect', width: '20', height: '8', x: '2', y: '2', rx: '2' },
			{ tag: 'rect', width: '20', height: '8', x: '2', y: '14', rx: '2' },
			{ tag: 'line', x1: '6', x2: '6.01', y1: '6', y2: '6' },
			{ tag: 'line', x1: '6', x2: '6.01', y1: '18', y2: '18' }
		]
	},
	'hard-drive': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'M10 16h.01' },
			{
				tag: 'path',
				d: 'M2.212 11.577a2 2 0 0 0-.212.896V18a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-5.527a2 2 0 0 0-.212-.896L18.55 5.11A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z'
			},
			{ tag: 'path', d: 'M21.946 12.013H2.054' },
			{ tag: 'path', d: 'M6 16h.01' }
		]
	},
	info: {
		style: 'stroke',
		elements: [
			{ tag: 'circle', cx: '12', cy: '12', r: '10' },
			{ tag: 'path', d: 'M12 16v-4' },
			{ tag: 'path', d: 'M12 8h.01' }
		]
	},
	'log-out': {
		style: 'stroke',
		elements: [
			{ tag: 'path', d: 'm16 17 5-5-5-5' },
			{ tag: 'path', d: 'M21 12H9' },
			{ tag: 'path', d: 'M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4' }
		]
	},
	'message-square-text': {
		style: 'stroke',
		elements: [
			{
				tag: 'path',
				d: 'M22 17a2 2 0 0 1-2 2H6.828a2 2 0 0 0-1.414.586l-2.202 2.202A.71.71 0 0 1 2 21.286V5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2z'
			},
			{ tag: 'path', d: 'M7 11h10' },
			{ tag: 'path', d: 'M7 15h6' },
			{ tag: 'path', d: 'M7 7h8' }
		]
	},
	'circle-alert': {
		style: 'stroke',
		elements: [
			{ tag: 'circle', cx: '12', cy: '12', r: '10' },
			{ tag: 'line', x1: '12', x2: '12', y1: '8', y2: '12' },
			{ tag: 'line', x1: '12', x2: '12.01', y1: '16', y2: '16' }
		]
	}
};

export function navIcon(id: NavIconId, selected: boolean): IconDef {
	return selected ? NAV_ICONS[id].solid : NAV_ICONS[id].outline;
}
