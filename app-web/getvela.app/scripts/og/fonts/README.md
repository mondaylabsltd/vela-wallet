# Card fallback fonts: Chinese, Japanese, Korean, and symbols

The social-preview cards (`/og/*.png`) are drawn by `rust/vela-og` in Inter,
which covers every script the site is written in except Han, kana and Hangul.
For those, each CJK locale has a pair of subsets here — `<locale>-400.ttf` and
`<locale>-700.ttf` — holding exactly the characters that locale's cards use:

| Locale  | Font         |
| ------- | ------------ |
| `zh`    | Noto Sans SC |
| `zh-TW` | Noto Sans TC |
| `zh-HK` | Noto Sans HK |
| `ja`    | Noto Sans JP |
| `ko`    | Noto Sans KR |

`symbols-400.ttf` (Noto Sans Symbols 2, one weight drawn for both) holds the
few symbols any card uses that neither Inter nor its CJK pair has — ✕, say.

They are cut by `rust/vela-og`'s own subsetter (`og_subset`), not fontTools:

```sh
bun run og:fonts
```

That reads every card's text, downloads the full fonts named in
`../fonts.json` once (into `node_modules/.cache/vela-og-fonts/`, each checked
against its pinned SHA-256), and rewrites the subsets. Run it after adding or
changing CJK titles or descriptions, and commit the result. If you forget, the
build stops and names the characters the subsets lack.

Licences: SIL Open Font License 1.1 — `OFL-NotoSansCJK.txt` (Copyright
2014-2021 Adobe, with Reserved Font Name 'Source'; the subsets keep the fonts'
own name, Noto Sans) and `OFL-NotoSansSymbols2.txt`.
