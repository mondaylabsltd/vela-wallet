// CSS module declarations for fresh checkouts (e.g. CI): locally these come from
// the gitignored, expo-generated expo-env.d.ts (via expo/types), which does not
// exist before the first `expo start`/`expo export`. Keep the shape identical to
// Expo's own declaration so the two merge cleanly when both are present.
declare module '*.module.css' {
  const classes: { readonly [key: string]: string };
  export default classes;
}
