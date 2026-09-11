# Bundled fonts

| Family | Files | Licence | Consumers |
| --- | --- | --- | --- |
| Plus Jakarta Sans | `PlusJakartaSans_{400Regular,500Medium,600SemiBold,700Bold}.ttf` | SIL OFL 1.1 (Tokotype) | `app-desktop` (spec 038: the UI face, loaded at startup so every machine renders the same text); the same faces the web ships via `@fontsource/plus-jakarta-sans` and iOS via `DesignSystem/Fonts/` |

The desktop cannot read woff2 (cosmic-text/fontdb loads TTF/OTF only), which
is why the TTFs live here rather than being taken from `node_modules`.
