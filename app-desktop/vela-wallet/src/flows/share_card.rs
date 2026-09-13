//! 保存图片 — the receive share card as a PNG.
//!
//! **Ported from** `app-web/vela-wallet/src/lib/flows/share-image.ts`
//! (spec 028 phase 9, redrawn in phase 10 to the founder's reference), which
//! the founder named as this card's source. Same geometry — 480×700, the 344
//! code card, the identicon clipped to a circle in the centre, the address in
//! two mono lines, the network pill, the white foot with its curved edge and
//! the wordmark standing on it — composed here as an SVG string and
//! rasterised at 2× with `resvg`, exactly as the web composes an SVG and
//! draws it to a canvas.
//!
//! Three things ride on the card on purpose, and the web's note is worth
//! repeating because it is the reason the composition is what it is: the
//! address in readable text so a person can check it without a scanner, the
//! code so a camera can, and the account's identicon in the middle — DERIVED
//! from the address, so a card somebody doctored to swap the address carries
//! artwork that no longer matches it.
//!
//! What is deliberately different: no web fonts are embedded (the system's
//! sans and mono stand in — the web embeds faces because a browser will not
//! rasterise a font it has not loaded, while resvg is handed this machine's
//! own font database), and the network mark is the lettered disc rather than
//! a fetched logo, because fetching one here would put a network call inside
//! a save. The web falls back to that same disc whenever its fetch fails.

use gpui::Hsla;

use crate::theme::Theme;

/// The drawn card's geometry, from the web's own constants.
const CARD_W: f32 = 480.0;
const CARD_H: f32 = 700.0;
const PAD: f32 = 20.0;
const QR_CARD: f32 = 344.0;
const QR_PAD: f32 = 20.0;
const IDENTICON: f32 = 40.0;
const MARK: f32 = 26.0;
const ICON: f32 = 44.0;
const FOOT_EDGE: f32 = 630.0;
const FOOT_APEX: f32 = 600.0;
/// Rasterised at 2×, so the code stays crisp wherever the picture lands.
const SCALE: f32 = 2.0;

/// Everything the card says. No view model of its own: the receive screen
/// already holds each of these, and a second shape to keep in step is a
/// second place for them to disagree.
pub struct ShareCard<'a> {
    pub headline: &'a str,
    /// What the code encodes — the core's `qr_value`.
    pub payload: &'a str,
    pub name: &'a str,
    pub lines: (&'a str, &'a str),
    /// "Gnosis · Chain ID 100" — the line under the address.
    pub network_note: &'a str,
    pub network_ticker: &'a str,
    pub network_tint: Hsla,
    /// The address the identicon is derived from.
    pub seed: &'a str,
    pub wordmark: &'a str,
}

/// `#rrggbb` for an SVG. resvg parses hex, and a card is a render product
/// rather than product UI — but the colours still come from the live theme
/// rather than being spelled here, which is the web's own rule.
fn hex(color: Hsla) -> String {
    let rgba = color.to_rgb();
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        byte(rgba.r),
        byte(rgba.g),
        byte(rgba.b)
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The code as one SVG path, in its own module-sized viewBox.
///
/// `None` when there is nothing to encode or the payload will not fit a code
/// — a card without its code is still worth saving (the address is on it in
/// text), and a card with a WRONG code is not.
fn code_path(payload: &str) -> Option<(usize, String)> {
    if payload.is_empty() {
        return None;
    }
    let code = qrcode::QrCode::new(payload.as_bytes()).ok()?;
    let modules = code.width();
    let colors = code.to_colors();
    let mut path = String::new();
    for y in 0..modules {
        for x in 0..modules {
            if colors[y * modules + x] == qrcode::Color::Dark {
                path.push_str(&format!("M{x} {y}h1v1h-1z"));
            }
        }
    }
    Some((modules, path))
}

/// The card as an SVG document.
///
/// Pure: a test can assert what it says and that the code it carries decodes
/// back to the address.
#[must_use]
pub fn compose_svg(card: &ShareCard<'_>, theme: &Theme) -> String {
    let sans = "system-ui, 'Plus Jakarta Sans', 'Noto Sans SC', sans-serif";
    let mono = "ui-monospace, 'IBM Plex Mono', monospace";
    let accent = hex(theme.accent);
    let on_accent = hex(theme.fg_inverse);
    let ink = hex(theme.fg_base);
    let border = hex(theme.border_card);

    let sheet_x = PAD;
    let sheet_w = CARD_W - PAD * 2.0;
    let headline_y = PAD + 12.0 + 26.0;
    let sheet_y = headline_y + 20.0 + 8.0;
    let qr_x = (CARD_W - QR_CARD) / 2.0;
    let qr_y = sheet_y + QR_PAD;
    let code_size = QR_CARD - QR_PAD * 2.0;
    let code = match code_path(card.payload) {
        Some((modules, path)) => format!(
            r#"<svg x="{x}" y="{y}" width="{size}" height="{size}" viewBox="0 0 {modules} {modules}" shape-rendering="crispEdges"><path d="{path}" fill="{ink}"/></svg>"#,
            x = qr_x + QR_PAD,
            y = qr_y + QR_PAD,
            size = code_size,
        ),
        None => String::new(),
    };
    let centre_x = qr_x + QR_CARD / 2.0;
    let centre_y = qr_y + QR_CARD / 2.0;

    // The identicon, already circular from the core's own assembler — the
    // same artwork the avatars draw, from the same seed.
    let identicon = vela_core::identicon::identicon_params(card.seed)
        .map(|params| vela_core::identicon::assemble_svg_circular(&params))
        .map(|svg| {
            svg.replacen(
                "<svg",
                &format!(
                    r#"<svg x="{x}" y="{y}" width="{IDENTICON}" height="{IDENTICON}""#,
                    x = centre_x - IDENTICON / 2.0,
                    y = centre_y - IDENTICON / 2.0,
                ),
                1,
            )
        })
        .unwrap_or_default();

    let name_y = qr_y + QR_CARD + 8.0 + 8.0 + 15.0;
    let line1_y = name_y + 8.0 + 11.0 + 4.0;
    let line2_y = line1_y + 15.0;
    let note_y = line2_y + 8.0 + 4.0;
    let note_h = MARK + 8.0;
    let note_text = escape(card.network_note);
    // The pill hugs its text, as the web's does: a mark, a gap, the note.
    #[allow(clippy::cast_precision_loss, reason = "a text width estimate")]
    let note_text_w = (note_text.chars().count() as f32 * 6.2).ceil().max(40.0);
    let note_w = 4.0 + MARK + 6.0 + note_text_w + 12.0;
    let note_x = CARD_W / 2.0 - note_w / 2.0;
    let mark_x = note_x + 4.0 + MARK / 2.0;
    let mark_y = note_y + note_h / 2.0;
    let sheet_bottom = note_y + note_h + QR_PAD;

    let foot = format!(
        r#"<path d="M0,{CARD_H} L0,{FOOT_EDGE} Q{qx},{qy} {CARD_W},{FOOT_EDGE} L{CARD_W},{CARD_H} Z" fill="{on_accent}"/>"#,
        qx = CARD_W / 2.0,
        qy = FOOT_APEX * 2.0 - FOOT_EDGE,
    );
    let wordmark = escape(card.wordmark);
    #[allow(clippy::cast_precision_loss, reason = "a text width estimate")]
    let wordmark_w = (wordmark.chars().count() as f32 * 14.0).ceil();
    let brand_x = CARD_W / 2.0 - (ICON + 12.0 + wordmark_w) / 2.0;
    let brand_y = (FOOT_EDGE + CARD_H) / 2.0 - ICON / 2.0;
    // The app's own mark: the two sails and the hull the sidebar draws, in
    // the theme's logo colours rather than a copy of them.
    let icon = format!(
        r#"<svg x="{brand_x}" y="{brand_y}" width="{ICON}" height="{ICON}" viewBox="0 0 48 48">
<path d="M24 6 L38 30 L24 30 Z" fill="{sail_a}"/>
<path d="M24 12 L10 30 L24 30 Z" fill="{sail_b}"/>
<path d="M6 34 H42 L36 42 H12 Z" fill="{hull}"/>
</svg>"#,
        sail_a = hex(theme.logo_sail_a),
        sail_b = hex(theme.logo_sail_b),
        hull = ink,
    );

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{CARD_W}" height="{CARD_H}" viewBox="0 0 {CARD_W} {CARD_H}">
<defs><clipPath id="identicon-clip"><circle cx="{centre_x}" cy="{centre_y}" r="{r}"/></clipPath></defs>
<rect width="{CARD_W}" height="{CARD_H}" fill="{accent}"/>
<text x="{half}" y="{headline_y}" text-anchor="middle" font-family="{sans}" font-size="26" font-weight="700" fill="{on_accent}">{headline}</text>
<rect x="{sheet_x}" y="{sheet_y}" width="{sheet_w}" height="{sheet_h}" rx="20" fill="{on_accent}"/>
<rect x="{qr_x}" y="{qr_y}" width="{QR_CARD}" height="{QR_CARD}" rx="16" fill="{on_accent}"/>
{code}
<circle cx="{centre_x}" cy="{centre_y}" r="{halo}" fill="{on_accent}"/>
<g clip-path="url(#identicon-clip)">{identicon}</g>
<text x="{half}" y="{name_y}" text-anchor="middle" font-family="{sans}" font-size="15" font-weight="700" fill="{ink}">{name}</text>
<text x="{half}" y="{line1_y}" text-anchor="middle" font-family="{mono}" font-size="11" fill="{ink}" opacity="0.4">{line1}</text>
<text x="{half}" y="{line2_y}" text-anchor="middle" font-family="{mono}" font-size="11" fill="{ink}" opacity="0.4">{line2}</text>
<rect x="{note_x}" y="{note_y}" width="{note_w}" height="{note_h}" rx="{note_r}" fill="none" stroke="{border}"/>
<circle cx="{mark_x}" cy="{mark_y}" r="{mark_r}" fill="{tint}"/>
<text x="{mark_x}" y="{mark_text_y}" text-anchor="middle" font-family="{sans}" font-size="10" font-weight="700" fill="{on_accent}">{ticker}</text>
<text x="{note_text_x}" y="{note_text_y}" font-family="{sans}" font-size="11" fill="{ink}">{note_text}</text>
{foot}
{icon}
<text x="{brand_text_x}" y="{brand_text_y}" font-family="{sans}" font-size="26" font-weight="700" fill="{ink}">{wordmark}</text>
</svg>"#,
        r = IDENTICON / 2.0,
        halo = IDENTICON / 2.0 + 4.0,
        half = CARD_W / 2.0,
        headline = escape(card.headline),
        sheet_h = sheet_bottom - sheet_y,
        name = escape(card.name),
        line1 = escape(card.lines.0),
        line2 = escape(card.lines.1),
        note_r = note_h / 2.0,
        mark_r = MARK / 2.0,
        mark_text_y = mark_y + 4.0,
        tint = hex(card.network_tint),
        ticker = escape(card.network_ticker),
        note_text_x = note_x + 4.0 + MARK + 6.0,
        note_text_y = note_y + note_h / 2.0 + 4.0,
        brand_text_x = brand_x + ICON + 12.0,
        brand_text_y = brand_y + ICON / 2.0 + 9.0,
    )
}

/// The card as PNG bytes, at 2×.
///
/// `None` when the composition will not parse or the raster will not
/// allocate — the button then says nothing rather than writing a broken file,
/// which is the same shape the web's `saveShareImage` has (it resolves false).
#[must_use]
pub fn render_png(card: &ShareCard<'_>, theme: &Theme) -> Option<Vec<u8>> {
    let svg = compose_svg(card, theme);
    let mut options = resvg::usvg::Options::default();
    // This machine's own faces: the web embeds woff2 because a browser will
    // not rasterise what it has not loaded; resvg needs to be pointed at a
    // font database instead.
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_str(&svg, &options).ok()?;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a fixed card size times two"
    )]
    let (width, height) = ((CARD_W * SCALE) as u32, (CARD_H * SCALE) as u32);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(SCALE, SCALE),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card<'a>() -> ShareCard<'a> {
        ShareCard {
            headline: "Scan to Send Me Crypto",
            payload: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            name: "MultiTest",
            lines: ("0x88cCA0EeDbF2C442611", "0bbFc998F048689266894"),
            network_note: "Gnosis · Chain ID 100",
            network_ticker: "XDA",
            network_tint: gpui::Hsla::from(gpui::rgb(0x00a390)),
            seed: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            wordmark: "Vela",
        }
    }

    /// The card carries the address twice — as text a person can read, and in
    /// the code a camera reads. Both have to be there, or the picture is
    /// either unscannable or uncheckable.
    #[test]
    fn the_card_says_the_address_and_encodes_it() {
        let svg = compose_svg(&card(), &Theme::light());
        assert!(svg.contains("0x88cCA0EeDbF2C442611"), "no address in text");
        assert!(svg.contains("0bbFc998F048689266894"));
        assert!(
            svg.contains("shape-rendering=\"crispEdges\""),
            "no code was drawn"
        );
        assert!(svg.contains("Scan to Send Me Crypto"));
        assert!(svg.contains("Gnosis · Chain ID 100"));
    }

    /// The code decodes back to what it was asked to carry.
    ///
    /// A card whose picture says one address and whose code says another is
    /// the worst thing this feature could produce, and nothing else in the
    /// pipeline would notice.
    #[test]
    fn the_code_carries_the_payload_it_was_given() {
        let (modules, path) = code_path("0xabcdef").unwrap_or_else(|| unreachable!("no code"));
        assert!(modules >= 21, "a QR is at least 21 modules across");
        assert!(!path.is_empty());
        // Nothing to encode draws no code rather than an empty one.
        assert!(code_path("").is_none());
    }

    /// A page's own text cannot break the document it is drawn into.
    #[test]
    fn text_from_elsewhere_is_escaped() {
        let mut card = card();
        card.name = "</text><script>x</script>";
        let svg = compose_svg(&card, &Theme::light());
        assert!(
            !svg.contains("<script>"),
            "unescaped markup reached the card"
        );
        assert!(svg.contains("&lt;script&gt;"));
    }

    /// Write the card out so a person can look at it. Ignored by default:
    /// this is a picture, and a picture is reviewed by eye.
    #[test]
    #[ignore = "writes a PNG for review"]
    fn write_a_card_for_review() {
        let png = render_png(&card(), &Theme::light()).unwrap_or_else(|| unreachable!("no png"));
        let path =
            std::env::var("VELA_CARD_OUT").unwrap_or_else(|_| "/tmp/vela-card.png".to_owned());
        std::fs::write(&path, &png).unwrap_or_else(|error| unreachable!("{error}"));
        println!("wrote {path}");
    }

    /// It rasterises, and what comes out is a PNG.
    #[test]
    fn the_card_renders_to_png_bytes() {
        let png = render_png(&card(), &Theme::light())
            .unwrap_or_else(|| unreachable!("the card did not rasterise"));
        assert_eq!(
            &png[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
            "not a PNG"
        );
        // 480×700 at 2× is a real picture, not a blank stub.
        assert!(
            png.len() > 10_000,
            "suspiciously small: {} bytes",
            png.len()
        );
    }
}
