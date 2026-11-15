# Third-party notices

## Inter (social-preview images)

The social-preview images (`/og/*.png`) are drawn at build time by
`rust/vela-og`, which embeds subsets of the Inter typeface, Copyright 2020 The
Inter Project Authors (https://github.com/rsms/inter), licensed under the SIL
Open Font License 1.1. The font files and the full license text are in
`rust/vela-og/fonts/` (`OFL.txt`). Characters Inter lacks are drawn from
subsets of Noto Sans SC, TC, HK, JP and KR (Copyright 2014-2021 Adobe, with
Reserved Font Name 'Source') and Noto Sans Symbols 2 (Copyright 2022 The Noto
Project Authors), also SIL OFL 1.1: `scripts/og/fonts/` holds the subsets and
their license texts.

## Heading slug character table

`scripts/markdown/slug-characters.js` retains the Unicode exclusion table from
`github-slugger` version 2.0.0 so existing heading IDs remain stable. Traversal,
text extraction and per-document collision handling are implemented locally.
The table is licensed under ISC:

Copyright (c) 2015, Dan Flettre <fletd01@yahoo.com>

Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted, provided that the above copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
