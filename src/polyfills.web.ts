/**
 * Web already provides crypto / btoa / atob / Buffer natively (browser +
 * Metro-web shims), so the native polyfills in `polyfills.ts` are unnecessary
 * here. This no-op keeps `import '@/polyfills'` valid on web without pulling
 * native-only packages (react-native-get-random-values) into the web bundle.
 */
export {};
