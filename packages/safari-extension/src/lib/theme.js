// Vela design tokens for the Safari extension UI (connect/sign sheets + popup).
//
// The SAME palette the app renders (src/constants/theme.ts) so the in-Safari UI is
// pixel-consistent with Vela Wallet — a mature product surface, not a prototype.
// Both light and dark are exact copies of the app's LIGHT_COLORS / DARK_COLORS.
//
// Theme matching: the app writes its color-scheme PREFERENCE ('auto'|'light'|'dark')
// into the shared account cache (vela.ext.account.json). The extension applies it so
// a user who FORCED dark in the app gets a dark sheet even on a light-mode system —
// exactly like the app. When the preference is 'auto' (or unknown), CSS falls back to
// the OS `prefers-color-scheme`, which is what the app's `resolveColorScheme` does too.

/** Exact Vela tokens (must stay in sync with src/constants/theme.ts). */
export const PALETTE = {
  light: {
    'fg-base': '#1A1A18', 'fg-muted': '#6E6B62', 'fg-subtle': '#8C887E', 'fg-inverse': '#FFFFFF',
    'bg-base': '#FAFAF8', 'bg-raised': '#FFFFFF', 'bg-sunken': '#F5F3EF',
    'accent': '#E8572A', 'accent-soft': '#FFF0EB',
    'success': '#2D8E5F', 'success-soft': '#EDFAF2',
    'warning': '#92600A',
    'error': '#C62828',
    'border': '#ECEBE4', 'border-strong': '#D8D6CE',
    'scrim': 'rgba(20,20,18,0.32)',
  },
  dark: {
    'fg-base': '#E8E6E1', 'fg-muted': '#9A9790', 'fg-subtle': '#85827A', 'fg-inverse': '#1A1A18',
    'bg-base': '#141412', 'bg-raised': '#1E1E1B', 'bg-sunken': '#0F0F0D',
    'accent': '#E8572A', 'accent-soft': '#2C1A12',
    'success': '#3DA872', 'success-soft': '#132A1E',
    'warning': '#D4A54A',
    'error': '#F87171',
    'border': '#2C2C28', 'border-strong': '#3E3E38',
    'scrim': 'rgba(0,0,0,0.5)',
  },
};

/** `--vela-*: value;` declarations for one scheme. */
export function varBlock(scheme) {
  const p = PALETTE[scheme] || PALETTE.light;
  return Object.keys(p).map((k) => `--vela-${k}: ${p[k]};`).join(' ');
}

/**
 * CSS that maps a `selector` to light-by-default, dark under `prefers-color-scheme`
 * when NOT explicitly forced light, and dark/light when the app FORCED a scheme via
 * a `[data-theme]` attribute. Mirrors the app's resolveColorScheme(pref, system):
 *   pref 'auto' → system;  pref 'light'|'dark' → forced.
 *
 * `selector` is the element carrying the vars (`:host` for the shadow sheet, `:root`
 * for the popup document). Emit once into the stylesheet.
 */
export function themeCss(selector) {
  return [
    `${selector} { ${varBlock('light')} }`,
    `@media (prefers-color-scheme: dark) { ${selector}:not([data-theme="light"]) { ${varBlock('dark')} } }`,
    `${selector}[data-theme="light"] { ${varBlock('light')} }`,
    `${selector}[data-theme="dark"] { ${varBlock('dark')} }`,
  ].join('\n');
}

/**
 * The value to put on the element's `data-theme` attribute given the app's stored
 * preference. Returns 'light' | 'dark' when the app forced a scheme (so it wins over
 * the OS), or '' when 'auto'/unknown (let the `prefers-color-scheme` media query
 * decide — identical to what the app shows for 'auto'). Never throws.
 */
export function dataThemeFor(pref) {
  return pref === 'light' || pref === 'dark' ? pref : '';
}
