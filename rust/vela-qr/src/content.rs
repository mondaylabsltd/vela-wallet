//! What a symbol carries: byte segments, each with the character set its bytes
//! are in.
//!
//! The crate carries no CJK or legacy single-byte tables (that is most of
//! what makes other readers large). It decodes to text itself only what needs
//! no table — ASCII, ISO-8859-1, UTF-8/16/32, CP437 — and names every other
//! character set by its WHATWG Encoding Standard label, which a browser's
//! `TextDecoder` takes as is and every native platform has a decoder for.
//! So nothing is refused for its script: Shift_JIS, GB18030, Big5, EUC-KR and
//! the ISO-8859 / Windows code pages all come out as bytes the platform turns
//! into text.

/// How a segment's bytes are to be read as text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Charset {
    Ascii,
    /// ISO-8859-1, exactly (not Windows-1252).
    Latin1,
    Utf8,
    Utf16Be,
    Utf16Le,
    Utf32Be,
    Utf32Le,
    /// IBM code page 437, the original QR default.
    Cp437,
    /// Needs the platform's decoder; the WHATWG label (`"shift_jis"`,
    /// `"gb18030"`, `"big5"`, `"euc-kr"`, `"iso-8859-2"`, `"windows-1251"`, …).
    Table(&'static str),
    /// ECI 899: binary data, not text.
    Binary,
    /// An ECI designator this crate does not know: the bytes, unread.
    Unknown(u32),
}

impl Charset {
    /// The character set an ECI designator assigns (AIM ITS/04-023).
    pub(crate) fn from_eci(eci: u32) -> Self {
        match eci {
            0 | 2 => Charset::Cp437,
            1 | 3 => Charset::Latin1,
            4 => Charset::Table("iso-8859-2"),
            5 => Charset::Table("iso-8859-3"),
            6 => Charset::Table("iso-8859-4"),
            7 => Charset::Table("iso-8859-5"),
            8 => Charset::Table("iso-8859-6"),
            9 => Charset::Table("iso-8859-7"),
            10 => Charset::Table("iso-8859-8"),
            11 => Charset::Table("iso-8859-9"),
            12 => Charset::Table("iso-8859-10"),
            13 => Charset::Table("iso-8859-11"),
            15 => Charset::Table("iso-8859-13"),
            16 => Charset::Table("iso-8859-14"),
            17 => Charset::Table("iso-8859-15"),
            18 => Charset::Table("iso-8859-16"),
            20 => Charset::Table("shift_jis"),
            21 => Charset::Table("windows-1250"),
            22 => Charset::Table("windows-1251"),
            23 => Charset::Table("windows-1252"),
            24 => Charset::Table("windows-1256"),
            25 => Charset::Utf16Be,
            26 => Charset::Utf8,
            27 | 170 => Charset::Ascii,
            28 => Charset::Table("big5"),
            29 | 32 => Charset::Table("gb18030"),
            30 => Charset::Table("euc-kr"),
            31 => Charset::Table("gbk"),
            33 => Charset::Utf16Le,
            34 => Charset::Utf32Be,
            35 => Charset::Utf32Le,
            899 => Charset::Binary,
            other => Charset::Unknown(other),
        }
    }

    /// The WHATWG label, for any charset (`None` for binary and unknown).
    #[must_use]
    pub fn label(self) -> Option<&'static str> {
        Some(match self {
            Charset::Ascii => "us-ascii",
            Charset::Latin1 => "iso-8859-1",
            Charset::Utf8 => "utf-8",
            Charset::Utf16Be => "utf-16be",
            Charset::Utf16Le => "utf-16le",
            Charset::Utf32Be => "utf-32be",
            Charset::Utf32Le => "utf-32le",
            Charset::Cp437 => "ibm437",
            Charset::Table(label) => label,
            Charset::Binary | Charset::Unknown(_) => return None,
        })
    }

    /// `bytes` as text, for the charsets this crate reads itself; strict —
    /// `None` on any invalid sequence, and always for `Table`/`Binary`/`Unknown`.
    #[must_use]
    pub fn decode(self, bytes: &[u8]) -> Option<String> {
        match self {
            Charset::Ascii => bytes
                .is_ascii()
                .then(|| bytes.iter().map(|&b| char::from(b)).collect()),
            Charset::Latin1 => Some(bytes.iter().map(|&b| char::from(b)).collect()),
            Charset::Utf8 => core::str::from_utf8(bytes).ok().map(str::to_owned),
            Charset::Utf16Be | Charset::Utf16Le => {
                if bytes.len() % 2 != 0 {
                    return None;
                }
                let units: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|c| {
                        if self == Charset::Utf16Be {
                            u16::from_be_bytes([c[0], c[1]])
                        } else {
                            u16::from_le_bytes([c[0], c[1]])
                        }
                    })
                    .collect();
                String::from_utf16(&units).ok()
            }
            Charset::Utf32Be | Charset::Utf32Le => {
                if bytes.len() % 4 != 0 {
                    return None;
                }
                bytes
                    .chunks_exact(4)
                    .map(|c| {
                        let a = [c[0], c[1], c[2], c[3]];
                        char::from_u32(if self == Charset::Utf32Be {
                            u32::from_be_bytes(a)
                        } else {
                            u32::from_le_bytes(a)
                        })
                    })
                    .collect()
            }
            Charset::Cp437 => Some(
                bytes
                    .iter()
                    .map(|&b| {
                        if b < 0x80 {
                            char::from(b)
                        } else {
                            CP437_HIGH[usize::from(b - 0x80)]
                        }
                    })
                    .collect(),
            ),
            Charset::Table(_) | Charset::Binary | Charset::Unknown(_) => None,
        }
    }
}

const CP437_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ',
    'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ',
    'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕',
    '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦',
    '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐',
    '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±',
    '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}',
];

/// One run of bytes in one character set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub charset: Charset,
    /// True when the symbol did not declare the charset (no ECI) and it was
    /// inferred from the bytes; [`Segment::candidates`] lists what to try.
    pub guessed: bool,
    pub bytes: Vec<u8>,
}

impl Segment {
    /// The charsets to try, in order, until one decodes the bytes strictly —
    /// just [`Segment::charset`] when declared. A guessed segment always ends
    /// in ISO-8859-1, which decodes anything, so it is never left unreadable.
    #[must_use]
    pub fn candidates(&self) -> Vec<Charset> {
        if self.guessed {
            guess(&self.bytes)
        } else {
            vec![self.charset]
        }
    }

    /// The text, if this crate can produce it without a platform decoder.
    #[must_use]
    pub fn text(&self) -> Option<String> {
        for c in self.candidates() {
            if let Charset::Table(_) = c {
                // A table charset ranked above: only the platform can say whether it fits.
                return None;
            }
            if let Some(t) = c.decode(&self.bytes) {
                return Some(t);
            }
        }
        None
    }
}

/// The position of a symbol in a structured-append sequence. Assembling the
/// sequence is the caller's; this crate only says the symbol is not the whole
/// message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    /// 0-based.
    pub index: u8,
    pub count: u8,
    pub parity: u8,
}

/// Everything one QR symbol carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code {
    pub segments: Vec<Segment>,
    /// `Some` when this symbol is one part of several.
    pub part: Option<Part>,
}

impl Code {
    /// The whole text, when every segment can be read without a platform
    /// decoder — which is always the case for what a wallet scans (ASCII and
    /// UTF-8). `None` means: decode the segments with the platform
    /// (`Segment::candidates`, `Charset::label`).
    #[must_use]
    pub fn text(&self) -> Option<String> {
        self.segments.iter().map(Segment::text).collect()
    }
}

/// The charsets undeclared bytes may be in, most likely first, ISO-8859-1
/// last. Structural checks only — no tables: UTF-8 if valid; GB18030 and
/// Shift_JIS if their byte grammar holds, ranked by what the bytes look like
/// (ZXing's `StringUtils.guessEncoding`, extended with GB18030).
fn guess(bytes: &[u8]) -> Vec<Charset> {
    if bytes.is_ascii() {
        return vec![Charset::Ascii];
    }
    if core::str::from_utf8(bytes).is_ok() {
        return vec![Charset::Utf8, Charset::Latin1];
    }
    if bytes.len() >= 2 && bytes.len() % 2 == 0 {
        if bytes.starts_with(&[0xFE, 0xFF]) {
            return vec![Charset::Utf16Be, Charset::Latin1];
        }
        if bytes.starts_with(&[0xFF, 0xFE]) {
            return vec![Charset::Utf16Le, Charset::Latin1];
        }
    }
    let gb = gb18030_shape(bytes);
    let sjis = shift_jis_shape(bytes);
    let mut out = Vec::with_capacity(3);
    if let Some(g) = &gb {
        // GB2312 hanzi are pairs of bytes ≥ 0xA1; Japanese in Shift_JIS is
        // mostly kanji with lead bytes below 0xA0, or runs of half-width
        // katakana. Latin-1 accents almost never all fall in such pairs, so
        // GB grammar holding outranks Latin-1.
        let japanese = sjis
            .as_ref()
            .is_some_and(|s| !g.all_gb2312 && (s.max_katakana_run >= 3 || s.max_double_run >= 3));
        if japanese {
            out.push(Charset::Table("shift_jis"));
            out.push(Charset::Table("gb18030"));
        } else {
            out.push(Charset::Table("gb18030"));
            if sjis.is_some() {
                out.push(Charset::Table("shift_jis"));
            }
        }
    } else if let Some(s) = &sjis {
        // Shift_JIS or Latin-1 (ZXing's heuristic): long kana/kanji runs, a
        // lone pair of katakana, or many non-letter Latin-1 bytes say
        // Shift_JIS; otherwise Latin-1, which then is the only candidate.
        let latin1_possible = !bytes.iter().any(|&b| (0x80..0xA0).contains(&b));
        let iso_high_other = bytes
            .iter()
            .filter(|&&b| (0xA0..0xC0).contains(&b) || b == 0xD7 || b == 0xF7)
            .count();
        let sjis_likely = s.max_katakana_run >= 3
            || s.max_double_run >= 3
            || (s.max_katakana_run == 2 && s.katakana == 2)
            || iso_high_other * 10 >= bytes.len();
        if latin1_possible && !sjis_likely {
            return vec![Charset::Latin1];
        }
        out.push(Charset::Table("shift_jis"));
    }
    // ISO-8859-1 decodes anything: always the last resort.
    out.push(Charset::Latin1);
    out
}

struct GbShape {
    all_gb2312: bool,
}

/// GB18030's byte grammar: ASCII, 2-byte (lead 81–FE, trail 40–7E/80–FE), or
/// 4-byte (81–FE, 30–39, 81–FE, 30–39).
fn gb18030_shape(bytes: &[u8]) -> Option<GbShape> {
    let (mut i, mut gb2312, mut other) = (0, 0usize, 0usize);
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            i += 1;
            continue;
        }
        if !(0x81..=0xFE).contains(&b) {
            return None;
        }
        let b2 = *bytes.get(i + 1)?;
        if (0x30..=0x39).contains(&b2) {
            let (b3, b4) = (*bytes.get(i + 2)?, *bytes.get(i + 3)?);
            if !(0x81..=0xFE).contains(&b3) || !(0x30..=0x39).contains(&b4) {
                return None;
            }
            other += 1;
            i += 4;
        } else if (0x40..=0xFE).contains(&b2) && b2 != 0x7F {
            if (0xA1..=0xF7).contains(&b) && (0xA1..=0xFE).contains(&b2) {
                gb2312 += 1;
            } else {
                other += 1;
            }
            i += 2;
        } else {
            return None;
        }
    }
    Some(GbShape {
        all_gb2312: gb2312 > 0 && other == 0,
    })
}

struct SjisShape {
    katakana: usize,
    max_katakana_run: usize,
    max_double_run: usize,
}

/// Shift_JIS's byte grammar, with the run statistics ZXing ranks it by.
fn shift_jis_shape(bytes: &[u8]) -> Option<SjisShape> {
    let (mut left, mut kana, mut double) = (0, 0usize, 0usize);
    let mut shape = SjisShape {
        katakana: 0,
        max_katakana_run: 0,
        max_double_run: 0,
    };
    for &b in bytes {
        if left > 0 {
            if b < 0x40 || b == 0x7F || b > 0xFC {
                return None;
            }
            left -= 1;
        } else if b == 0x80 || b == 0xA0 || b > 0xEF {
            return None;
        } else if (0xA1..0xE0).contains(&b) {
            double = 0;
            kana += 1;
            shape.katakana += 1;
            shape.max_katakana_run = shape.max_katakana_run.max(kana);
        } else if b > 0x7F {
            left = 1;
            kana = 0;
            double += 1;
            shape.max_double_run = shape.max_double_run.max(double);
        } else {
            kana = 0;
            double = 0;
        }
    }
    (left == 0).then_some(shape)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guessed(bytes: &[u8]) -> Segment {
        Segment {
            charset: guess(bytes)[0],
            guessed: true,
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn utf8_and_ascii_need_no_platform() {
        assert_eq!(
            guessed("中文 ✓".as_bytes()).text().as_deref(),
            Some("中文 ✓")
        );
        assert_eq!(guessed(b"0xabc").text().as_deref(), Some("0xabc"));
    }

    #[test]
    fn gbk_chinese_is_ranked_gb18030() {
        // "你好，世界" in GBK.
        let gbk = [0xC4, 0xE3, 0xBA, 0xC3, 0xA3, 0xAC, 0xCA, 0xC0, 0xBD, 0xE7];
        assert_eq!(guess(&gbk)[0], Charset::Table("gb18030"));
        assert_eq!(*guess(&gbk).last().unwrap(), Charset::Latin1);
        assert_eq!(guessed(&gbk).text(), None, "the platform decides");
    }

    #[test]
    fn shift_jis_japanese_is_ranked_shift_jis() {
        // "日本語のテキスト" in Shift_JIS: kanji leads 0x93/0x96/0x8C, kana 0x82/0x83.
        let sjis = [
            0x93, 0xFA, 0x96, 0x7B, 0x8C, 0xEA, 0x82, 0xCC, 0x83, 0x65, 0x83, 0x4C, 0x83, 0x58,
            0x83, 0x67,
        ];
        assert_eq!(guess(&sjis)[0], Charset::Table("shift_jis"));
    }

    #[test]
    fn gbk_mixed_with_ascii_is_chinese() {
        // "价格: 100元" in GBK.
        let mut b = vec![0xBC, 0xDB, 0xB8, 0xF1];
        b.extend_from_slice(b": 100");
        b.extend_from_slice(&[0xD4, 0xAA]);
        assert_eq!(guess(&b)[0], Charset::Table("gb18030"));
    }

    #[test]
    fn valid_utf8_wins_even_when_gbk_could_also_fit() {
        // An inherent ambiguity: GBK 元 (D4 AA) is also UTF-8 "Ԫ", as UTF-8
        // "é" (C3 A9) is also a GB2312 hanzi. Valid UTF-8 is read as UTF-8 —
        // what ZXing and zxing-cpp do, and what keeps accented UTF-8 text right.
        assert_eq!(guess(&[0xD4, 0xAA])[0], Charset::Utf8);
        assert_eq!(guess("Café".as_bytes())[0], Charset::Utf8);
    }

    #[test]
    fn latin1_accents_stay_latin1() {
        assert_eq!(
            guessed(b"Caf\xE9 cr\xE8me").text().as_deref(),
            Some("Café crème")
        );
    }

    #[test]
    fn declared_charsets_decode_or_defer() {
        assert_eq!(
            Charset::from_eci(26).decode("€".as_bytes()).as_deref(),
            Some("€")
        );
        assert_eq!(
            Charset::from_eci(25)
                .decode(&[0x4E, 0x2D, 0x65, 0x87])
                .as_deref(),
            Some("中文")
        );
        assert_eq!(
            Charset::from_eci(2).decode(&[0x80, 0xE1]).as_deref(),
            Some("Çß")
        );
        assert_eq!(Charset::from_eci(28), Charset::Table("big5"));
        assert_eq!(Charset::from_eci(20).decode(&[0x93, 0xFA]), None);
        assert_eq!(Charset::from_eci(899), Charset::Binary);
        assert_eq!(Charset::from_eci(12345), Charset::Unknown(12345));
        assert_eq!(Charset::from_eci(26).decode(&[0xFF]), None, "strict");
    }
}
