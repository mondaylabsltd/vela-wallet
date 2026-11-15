use super::*;

fn bold() -> Font<'static> {
    Font::parse(INTER_BOLD).unwrap()
}

fn card<'a>(section: &'a str, title: &'a str, description: &'a str, meta: &'a str) -> Card<'a> {
    Card {
        site: "Vela Wallet",
        domain: "getvela.app",
        section,
        title,
        description,
        meta,
    }
}

#[test]
fn maps_every_script_the_non_cjk_locales_use() {
    let f = bold();
    for c in "Aa Zz 09 ’“”—–…'\"?!,.:;()&%$€é ĞğİıŞş ÀÂÇÉÈÊËÎÏÔÙÛÜŸ ẠẢẤầẩẫậắằẳẵặẹẻẽếềểễệịọỏốồổỗộớờởỡợụủứừửữựỳỵỷỹđ ЖжЩщЁё ΑΩαω ẞß".chars() {
        assert!(f.glyph_id(c).is_some() || c == ' ', "no glyph for {c:?}");
    }
    assert!(f.glyph_id('金').is_none());
}

#[test]
fn kerns_the_pairs_that_need_it() {
    let f = bold();
    let (a, v) = (f.glyph_id('A').unwrap(), f.glyph_id('V').unwrap());
    assert!(f.kerning(a, v) < 0.0, "A–V should tighten");
    let (h, i) = (f.glyph_id('H').unwrap(), f.glyph_id('I').unwrap());
    assert_eq!(f.kerning(h, i), 0.0);
}

#[test]
fn outlines_have_area() {
    let f = bold();
    for c in ['O', 'é', '“', 'g', 'Ж'] {
        assert!(
            !f.outline(f.glyph_id(c).unwrap()).segments.is_empty(),
            "{c:?}"
        );
    }
}

#[test]
fn breaks_between_words_and_between_cjk_characters() {
    assert_eq!(break_units("Why  Vela"), vec!["Why", " ", "Vela"]);
    // Closing punctuation stays with the character before it, opening with
    // the one after; a Latin word inside CJK text stays whole.
    assert_eq!(
        break_units("为什么用「Vela」？"),
        vec!["为", "什", "么", "用", "「Vela」？"]
    );
    assert_eq!(
        break_units("あいう、えお"),
        vec!["あ", "い", "う、", "え", "お"]
    );
    // Korean breaks between words, not syllables.
    assert_eq!(
        break_units("연결해 두었습니다."),
        vec!["연결해", " ", "두었습니다."]
    );
}

#[test]
fn wraps_and_ellipsizes() {
    let faces = [bold()];
    let (short, cut) = wrap(
        &faces,
        60.0,
        "Why Vela has no seed phrase",
        CONTENT_WIDTH,
        3,
    );
    assert_eq!(short, vec!["Why Vela has no seed phrase"]);
    assert!(!cut);
    let long = "word ".repeat(80);
    let (lines, cut) = wrap(&faces, 60.0, &long, CONTENT_WIDTH, 3);
    assert!(cut);
    assert_eq!(lines.len(), 3);
    assert!(lines[2].ends_with('…'));
    // The ellipsis follows a whole word.
    let (cut, _) = wrap(&faces, 60.0, &"wordy ".repeat(80), CONTENT_WIDTH, 2);
    assert!(cut[1].ends_with("wordy…"), "{:?}", cut[1]);
    for l in &lines {
        assert!(Run::new(&faces, 60.0, l).width() <= CONTENT_WIDTH);
    }
    let (giant, _) = wrap(&faces, 60.0, &"x".repeat(200), CONTENT_WIDTH, 4);
    assert_eq!(giant.len(), 4);
}

#[test]
fn coverage_of_a_square_is_exact() {
    let mut c = Coverage::new(8, 8);
    let p = |x: f32, y: f32| Point { x, y };
    c.fill(&[
        Segment::Line(p(2.0, 2.0), p(6.0, 2.0)),
        Segment::Line(p(6.0, 2.0), p(6.0, 6.0)),
        Segment::Line(p(6.0, 6.0), p(2.0, 6.0)),
        Segment::Line(p(2.0, 6.0), p(2.0, 2.0)),
    ]);
    let a = c.finish();
    let total: f32 = a.iter().sum();
    assert!((total - 16.0).abs() < 1e-3, "area {total}");
    assert_eq!(a[3 * 8 + 3], 1.0);
    assert_eq!(a[0], 0.0);
    let mut c = Coverage::new(4, 1);
    c.fill(&[
        Segment::Line(p(0.0, 0.0), p(1.5, 0.0)),
        Segment::Line(p(1.5, 0.0), p(1.5, 1.0)),
        Segment::Line(p(1.5, 1.0), p(0.0, 1.0)),
        Segment::Line(p(0.0, 1.0), p(0.0, 0.0)),
    ]);
    let a = c.finish();
    assert!((a[1] - 0.5).abs() < 1e-4, "{a:?}");
}

#[test]
fn refuses_what_the_fonts_cannot_draw() {
    assert_eq!(
        check(&card("文档", "为什么 Vela 没有助记词", "", ""), &[]),
        Err(Refusal::Missing("文档为什么没有助记词".into()))
    );
    assert!(check(&card("Docs", "Почему у Vela нет сид-фразы", "", ""), &[]).is_ok());
}

#[test]
fn a_subset_draws_exactly_what_the_original_drew() {
    let text = "Vela AV “quoted” — é";
    let cut = subset::subset(INTER_BOLD, text).unwrap();
    assert!(cut.len() < INTER_BOLD.len() / 10, "{} bytes", cut.len());
    let (orig, sub) = (bold(), Font::parse(&cut).unwrap());
    assert_eq!(sub.units_per_em, orig.units_per_em);
    assert_eq!(sub.ascender, orig.ascender);
    for c in text.chars().filter(|c| *c != ' ') {
        let (a, b) = (orig.glyph_id(c).unwrap(), sub.glyph_id(c).unwrap());
        assert_eq!(orig.advance(a), sub.advance(b), "{c:?}");
        let (oa, ob) = (orig.outline(a).segments, sub.outline(b).segments);
        assert_eq!(oa.len(), ob.len(), "{c:?}");
        for (x, y) in oa.iter().zip(&ob) {
            assert_eq!(x.points(), y.points(), "{c:?}");
        }
    }
    assert!(sub.glyph_id('Z').is_none());
    // Deterministic: the same input cuts to the same bytes.
    assert_eq!(subset::subset(INTER_BOLD, text).unwrap(), cut);
    // A composite glyph (é = e + ´) brings its components along.
    assert!(!sub.outline(sub.glyph_id('é').unwrap()).segments.is_empty());
}

#[test]
fn png_is_well_formed() {
    let png = render(
        &card(
            "Blog",
            "Pulse users aren’t trapped because of passkeys",
            "Why the wallet shutting down is not a passkey problem.",
            "Shelchin · August 8, 2026",
        ),
        &[],
    )
    .unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&png[12..16], b"IHDR");
    let mut at = 8;
    while at < png.len() {
        let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
        let crc = u32::from_be_bytes(png[at + 8 + len..at + 12 + len].try_into().unwrap());
        assert_eq!(png::crc32(&png[at + 4..at + 8 + len]), crc);
        at += 12 + len;
    }
    assert!(png.len() < 250_000, "{} bytes", png.len());
}

/// `VELA_OG_SAMPLE=<dir>` writes sample cards there; with `VELA_OG_CJK=<dir>`
/// (NotoSans{SC,JP,KR}-{400,700}.ttf) the CJK ones too, through `subset`.
#[test]
fn samples() {
    let Ok(dir) = std::env::var("VELA_OG_SAMPLE") else {
        return;
    };
    let latin: [(&str, Card); 4] = [
        ("blog", card("Blog", "Pulse users aren’t trapped because of passkeys", "Pulse is shutting down, and its users are worried their passkeys lock them in. They don’t — here is why, and how to move your funds.", "Shelchin · August 8, 2026")),
        ("docs", card("Docs", "Clear signing", "Vela decodes transactions into plain language before you approve them — intent, amounts, addresses, and risk — instead of opaque hex.", "")),
        ("docs-long", card("Docs", "Vela is in alpha — what that means, and what we're asking of you while we get it right", "The honest version: what works, what does not yet, and how you can help.", "")),
        ("ru", card("Документация", "Почему у Vela нет сид-фразы", "Короткая версия: ключ живёт в вашем устройстве, а не на бумажке.", "")),
    ];
    for (name, c) in &latin {
        std::fs::write(format!("{dir}/og-{name}.png"), render(c, &[]).unwrap()).unwrap();
    }
    let Ok(cjk) = std::env::var("VELA_OG_CJK") else {
        return;
    };
    let cards = [
        ("zh", "SC", card("文档", "我们为什么做 Vela", "长一点的版本——那十二个单词到底该放在哪儿、通行密钥改变了什么、我们在自己已经在用的钱包里不能接受什么，以及我们选择的那个代价。", "")),
        ("ja", "JP", card("ドキュメント", "Vela にシードフレーズがない理由", "鍵はあなたのデバイスの中にあり、紙の上にはありません。パスキーが変えたことと、私たちが選んだ代償について。", "")),
        ("ko", "KR", card("문서", "Vela에 시드 문구가 없는 이유", "키는 종이가 아니라 기기 안에 있습니다. 패스키가 바꾼 것과 우리가 선택한 대가에 대하여.", "")),
    ];
    for (name, region, c) in &cards {
        let text = format!(
            "{}{}{}{}{}…",
            c.section, c.title, c.description, c.meta, c.site
        );
        let cut: Vec<(u16, Vec<u8>)> = [400u16, 700]
            .iter()
            .map(|w| {
                let full = std::fs::read(format!("{cjk}/NotoSans{region}-{w}.ttf")).unwrap();
                (*w, subset::subset(&full, &text).unwrap())
            })
            .collect();
        let fb: Vec<(u16, &[u8])> = cut.iter().map(|(w, b)| (*w, b.as_slice())).collect();
        for (w, bytes) in &cut {
            std::fs::write(format!("{dir}/subset-{name}-{w}.ttf"), bytes).unwrap();
        }
        std::fs::write(format!("{dir}/og-{name}.png"), render(c, &fb).unwrap()).unwrap();
    }
}

#[test]
fn requests_are_six_fields() {
    assert!(parse_request(b"a\0b\0c\0d\0e\0f").is_some());
    assert!(parse_request(b"a\0b\0c\0d\0e").is_none());
    assert!(parse_request(b"a\0b\0c\0d\0e\0f\0g").is_none());
    assert!(parse_request(&[0xff, 0, 0, 0, 0, 0]).is_none());
}

#[test]
#[ignore]
fn profile() {
    use std::time::Instant;
    let t = Instant::now();
    let canvas = Canvas::paper();
    let paper = t.elapsed();
    let rgb = canvas.rgb();
    let t = Instant::now();
    let png = png::encode_rgb(WIDTH as u32, HEIGHT as u32, &rgb);
    let enc = t.elapsed();
    let t = Instant::now();
    let _ = render(
        &card(
            "Docs",
            "Clear signing",
            "Vela decodes transactions into plain language before you approve them.",
            "",
        ),
        &[],
    )
    .unwrap();
    eprintln!(
        "paper {paper:?} encode {enc:?} ({} KB) whole {:?}",
        png.len() / 1024,
        t.elapsed()
    );
}
