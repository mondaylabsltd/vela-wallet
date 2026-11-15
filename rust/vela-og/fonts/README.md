# Fonts

`Inter-Regular.ttf` and `Inter-Bold.ttf` are Inter 4 (Google Fonts build, v18),
licensed under the SIL Open Font License 1.1 — see `OFL.txt`. They are
subsets, made with fontTools so the module stays small:

```sh
U="U+0020-007E,U+00A0-024F,U+0259,U+02B0-02FF,U+0370-03FF,U+0400-04FF,U+0500-052F,U+1E00-1EFF,U+2010-2027,U+2030-205E,U+20A0-20C0,U+2100-214F,U+2190-21FF,U+2200-22FF,U+2300-23FF,U+2460-24FF,U+25A0-25FF,U+2600-26FF,U+2700-27BF,U+FFFD"
pyftsubset Inter-<weight>.ttf --unicodes="$U" --layout-features=kern --no-hinting \
  --desubroutinize --drop-tables+=STAT,gasp,prep,GDEF,GSUB --name-IDs='*'
```

Latin (with Latin Extended and Vietnamese), Greek, Cyrillic, typographic
punctuation, currency signs, arrows and the symbols Inter draws (✓ → ≠ ①, …),
and the kerning feature: every script the
site's non-CJK locales are written in. Titles are NFC-normalized before
drawing (precomposed letters; there is no mark positioning here). A title
with a character no font covers is refused — the build names the character —
never drawn as `?` or a missing-glyph box. Characters Inter lacks — CJK, and
symbols such as ✕ — come from fallback subsets the site registers
(app-web/getvela.app/scripts/og/fonts/).
