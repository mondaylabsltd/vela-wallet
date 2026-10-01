//! 保存图片 — the receive share card as a PNG.
//!
//! **Ported from** `app-web/vela-wallet/src/lib/flows/share-image.ts`, the
//! card's source (recomposed 2026-09-27 after the WeChat Pay collection card
//! the founder holds it against). Same geometry — the web's `SHARE_CARD`,
//! copied below — composed here as an SVG string and rasterised at 2× with
//! `resvg`, exactly as the web composes an SVG and draws it to a canvas.
//!
//! The composition, top to bottom: the app icon's orange field with the
//! headline and the one network this address may be paid on; a white sheet
//! with generous orange all round, holding the code — the NETWORK's logo on a
//! plate in its centre — and under the code the account: its identicon on the
//! left, the name and the whole address in two mono lines beside it; then the
//! field closes over a white foot in one curve that dips at the centre, and
//! the canonical app icon and the wordmark stand on the white.
//!
//! Three things ride on the card on purpose: the address in readable text so
//! a person can check it without a scanner, the code so a camera can, and the
//! account's identicon — DERIVED from the address, so a card somebody
//! doctored to swap the address carries artwork that no longer matches the
//! characters printed beside it.
//!
//! The code is encoded at level H: the logo plate covers about 7% of it, and
//! a picture that travels through chat apps is recompressed on the way.
//!
//! What is deliberately different from the web: the faces are the app's own
//! bundled Plus Jakarta Sans and the host's mono (the web embeds woff2 into
//! the document; resvg is handed a font database instead), text is measured
//! by resvg's own layout rather than a canvas, and the logo — fetched by the
//! caller, PNG or WebP — is decoded with `image` and composited onto the
//! pixmap after the vector pass, because this build of resvg carries no
//! raster decoders.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use gpui::Hsla;
use resvg::tiny_skia;
use resvg::usvg;
use resvg::usvg::fontdb;

/// The card's geometry in px at 1× — the web's `SHARE_CARD`, one for one.
const WIDTH: f32 = 480.0;
const TOP: f32 = 52.0;
const HEADLINE_SIZE: f32 = 32.0;
const HEADLINE_MIN_SIZE: f32 = 26.0;
const HEADLINE_LEADING: f32 = 1.25;
/// Widest a line of text on the orange may run.
const TEXT_WIDTH: f32 = 400.0;
const NOTE_GAP: f32 = 10.0;
const NOTE_SIZE: f32 = 15.0;
const NOTE_LINE: f32 = 20.0;
const SHEET_GAP: f32 = 28.0;
const SHEET_WIDTH: f32 = 320.0;
const SHEET_RADIUS: f32 = 20.0;
const SHEET_PAD: f32 = 48.0;
const SHEET_PAD_BOTTOM: f32 = 40.0;
const QR: f32 = 224.0;
/// The white plate the network logo sits on, in the code's centre.
const PLATE: f32 = 60.0;
const PLATE_RADIUS: f32 = 16.0;
const LOGO: f32 = 44.0;
const IDENTITY_GAP: f32 = 26.0;
const IDENTICON: f32 = 48.0;
const IDENTITY_TEXT_GAP: f32 = 12.0;
const NAME_SIZE: f32 = 17.0;
const NAME_LINE: f32 = 22.0;
const ADDRESS_SIZE: f32 = 12.5;
const ADDRESS_LINE: f32 = 17.0;
const NAME_ADDRESS_GAP: f32 = 3.0;
/// Sheet bottom to where the curve leaves the card's edges.
const CURVE_GAP: f32 = 52.0;
/// How far the curve dips at the centre.
const CURVE_DEPTH: f32 = 32.0;
/// The curve's lowest point to the card's bottom.
const FOOT: f32 = 112.0;
const ICON: f32 = 52.0;
const ICON_GAP: f32 = 12.0;
const WORDMARK_SIZE: f32 = 32.0;
/// Rasterised at 2×, so the code stays crisp wherever the picture lands.
const SCALE: f32 = 2.0;

/// The card's colours. A render product, so fixed rather than themed: the
/// picture is saved once and viewed anywhere, and one saved in dark mode
/// must not be a different card. The field is the APP ICON's own orange
/// (founder, 2026-08-15: the icon's #F46D50, not the UI accent); paper and
/// ink are the web's `--color-onAccent` and `--color-fixed-shadowInk`.
const FIELD: &str = "#f46d50";
const PAPER: &str = "#ffffff";
const INK: &str = "#1a1a18";

const SANS: &str = "'Plus Jakarta Sans', 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', 'Noto Sans CJK SC', sans-serif";

/// CJK faces a glyph the bundled Latin face lacks falls back to, in order —
/// platform sans, never a serif (the founder's CJK rule). resvg's own
/// fallback takes the first face in the database that has the glyph, which
/// on a Mac can be Songti.
const CJK_SANS: [&str; 12] = [
    "PingFang SC",
    "PingFang TC",
    "PingFang HK",
    "Hiragino Sans GB",
    "Hiragino Sans",
    "Apple SD Gothic Neo",
    "Microsoft YaHei",
    "Malgun Gothic",
    "Yu Gothic",
    "Noto Sans CJK SC",
    "Noto Sans SC",
    "Source Han Sans SC",
];

/// The canonical application icon — THE mark every platform's icon is
/// rendered from. Read from the file itself rather than redrawn: the card
/// this replaced drew a sailboat of its own, and the founder saw the
/// difference at once.
const APP_ICON_SVG: &str = include_str!("../../../../docs/design/icon/app-icon.svg");

/// How long the logo fetch may hold up a save before the lettered disc
/// stands in.
const LOGO_TIMEOUT: Duration = Duration::from_secs(4);

/// Everything the card says. No view model of its own: the receive screen
/// already holds each of these, and a second shape to keep in step is a
/// second place for them to disagree.
pub struct ShareCard<'a> {
    pub headline: &'a str,
    /// What the code encodes — the core's `qr_value`.
    pub payload: &'a str,
    pub name: &'a str,
    pub lines: (&'a str, &'a str),
    /// "{{network}} payments only" — the line under the headline.
    pub network_note: &'a str,
    /// The lettered disc's letters, when there is no logo.
    pub network_ticker: &'a str,
    pub network_tint: Hsla,
    /// The network's logo as fetched (PNG or WebP); `None` draws the disc.
    pub network_logo: Option<&'a [u8]>,
    /// The address the identicon is derived from.
    pub seed: &'a str,
    pub wordmark: &'a str,
}

/// Width of `text` in px: the text, its size, its weight, and whether it is
/// set in the mono face.
type Measure<'m> = &'m dyn Fn(&str, f32, u16, bool) -> f32;

/// A measure for when there is no font database (the pure composer's
/// tests): per-character advances close to the bundled faces, wide for CJK.
/// The web's `estimateWidth`.
fn estimate_width(text: &str, size: f32, weight: u16, mono: bool) -> f32 {
    #[allow(clippy::cast_precision_loss, reason = "a text width estimate")]
    if mono {
        return text.chars().count() as f32 * size * 0.6;
    }
    let em: f32 = text
        .chars()
        .map(|c| match c {
            c if u32::from(c) >= 0x2e80 => 1.0,
            ' ' => 0.27,
            'A'..='Z' => 0.68,
            'm' | 'w' => 0.86,
            'i' | 'l' | 'j' | 't' | 'f' | '.' | ',' | '\'' | '!' | '|' | ':' | ';' => 0.3,
            _ => 0.57,
        })
        .sum();
    em * size * if weight >= 700 { 1.04 } else { 1.0 }
}

/// The headline set to the card: one line at the largest size from 32 down
/// to 26 that fits, else two lines split where the halves come out closest
/// in width (at a space when there is one, anywhere in CJK), shrunk until
/// the longer half fits. The web's `fitHeadline`.
fn fit_headline(text: &str, measure: Measure<'_>) -> (f32, Vec<String>) {
    let whole = measure(text, HEADLINE_SIZE, 700, false);
    if whole <= TEXT_WIDTH {
        return (HEADLINE_SIZE, vec![text.to_owned()]);
    }
    let shrunk = (HEADLINE_SIZE * TEXT_WIDTH / whole).floor();
    if shrunk >= HEADLINE_MIN_SIZE {
        return (shrunk, vec![text.to_owned()]);
    }
    let chars: Vec<char> = text.chars().collect();
    let has_space = chars.contains(&' ');
    let mut best = (text.to_owned(), String::new());
    let mut best_width = f32::INFINITY;
    for i in 1..chars.len() {
        if has_space && chars[i] != ' ' {
            continue;
        }
        let first = chars[..i].iter().collect::<String>().trim().to_owned();
        let second = chars[i..].iter().collect::<String>().trim().to_owned();
        if first.is_empty() || second.is_empty() {
            continue;
        }
        let width = measure(&first, HEADLINE_SIZE, 700, false).max(measure(
            &second,
            HEADLINE_SIZE,
            700,
            false,
        ));
        if width < best_width {
            best = (first, second);
            best_width = width;
        }
    }
    let size = HEADLINE_SIZE.min((HEADLINE_SIZE * TEXT_WIDTH / best_width).floor());
    (size, vec![best.0, best.1])
}

/// `text` cut to `width` with an ellipsis, or whole when it fits.
fn truncate(text: &str, width: f32, fits: impl Fn(&str) -> f32) -> String {
    if fits(text) <= width {
        return text.to_owned();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while chars.len() > 1 && fits(&format!("{}…", chars.iter().collect::<String>())) > width {
        chars.pop();
    }
    format!("{}…", chars.iter().collect::<String>().trim_end())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Two decimals, so a coordinate prints as a coordinate.
fn r(value: f32) -> f32 {
    (value * 100.0).round() / 100.0
}

/// The baseline that centres a line of `size` text on `centre`.
fn baseline(centre: f32, size: f32) -> f32 {
    r(centre + size * 0.35)
}

/// `#rrggbb` for an SVG.
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

fn mono_family() -> String {
    format!("'{}', monospace", crate::theme::font_mono())
}

/// The code as one SVG path, in its own module-sized viewBox, at level H.
///
/// `None` when there is nothing to encode or the payload will not fit a code
/// — a card without its code is still worth saving (the address is on it in
/// text), and a card with a WRONG code is not.
fn code_path(payload: &str) -> Option<(usize, String)> {
    if payload.is_empty() {
        return None;
    }
    let code =
        qrcode::QrCode::with_error_correction_level(payload.as_bytes(), qrcode::EcLevel::H).ok()?;
    let modules = code.width();
    let colors = code.to_colors();
    let mut path = String::new();
    for y in 0..modules {
        let mut x = 0;
        while x < modules {
            if colors[y * modules + x] != qrcode::Color::Dark {
                x += 1;
                continue;
            }
            // Consecutive darks as one run: per-module squares leave
            // hairline seams from rounding, and a seamed code photographs
            // badly.
            let mut run = 1;
            while x + run < modules && colors[y * modules + x + run] == qrcode::Color::Dark {
                run += 1;
            }
            path.push_str(&format!("M{x} {y}h{run}v1h-{run}z"));
            x += run;
        }
    }
    Some((modules, path))
}

/// The canonical icon's drawing, re-rooted at `x`/`y` and `size`: the file's
/// own prolog, comment and 68-unit size are dropped, its viewBox kept.
fn app_icon(x: f32, y: f32, size: f32) -> String {
    let body = APP_ICON_SVG
        .find("<svg")
        .and_then(|start| {
            let open_end = start + APP_ICON_SVG[start..].find('>')?;
            let close = APP_ICON_SVG.rfind("</svg>")?;
            Some(&APP_ICON_SVG[open_end + 1..close])
        })
        .unwrap_or_default();
    format!(
        r#"<svg x="{x}" y="{y}" width="{size}" height="{size}" viewBox="0 0 68 68">{body}</svg>"#
    )
}

/// A composed card: the document, and what the raster pass needs from it.
struct Composed {
    svg: String,
    height: f32,
    /// The code's centre, where the logo is composited.
    logo_centre: (f32, f32),
}

fn compose(card: &ShareCard<'_>, measure: Measure<'_>, logo: bool) -> Composed {
    let mono = mono_family();
    let half = WIDTH / 2.0;

    // The orange: headline, then the network it may be paid on.
    let (headline_size, headline_lines) = fit_headline(card.headline, measure);
    let headline_line = headline_size * HEADLINE_LEADING;
    let mut headline = String::new();
    for (i, line) in headline_lines.iter().enumerate() {
        #[allow(clippy::cast_precision_loss, reason = "one or two lines")]
        let y = baseline(TOP + headline_line * (i as f32 + 0.5), headline_size);
        headline.push_str(&format!(
            r#"<text x="{half}" y="{y}" text-anchor="middle" font-family="{SANS}" font-size="{headline_size}" font-weight="700" fill="{PAPER}">{}</text>
"#,
            escape(line)
        ));
    }
    #[allow(clippy::cast_precision_loss, reason = "one or two lines")]
    let note_top = TOP + headline_line * headline_lines.len() as f32 + NOTE_GAP;
    let note_width = measure(card.network_note, NOTE_SIZE, 500, false);
    let note_size = if note_width <= TEXT_WIDTH {
        NOTE_SIZE
    } else {
        (NOTE_SIZE * TEXT_WIDTH / note_width).floor().max(11.0)
    };
    let note = format!(
        r#"<text x="{half}" y="{y}" text-anchor="middle" font-family="{SANS}" font-size="{note_size}" font-weight="500" fill="{PAPER}">{text}</text>"#,
        y = baseline(note_top + NOTE_LINE / 2.0, note_size),
        text = escape(card.network_note),
    );

    // The sheet and its code.
    let sheet_x = (WIDTH - SHEET_WIDTH) / 2.0;
    let sheet_y = note_top + NOTE_LINE + SHEET_GAP;
    let qr_x = (WIDTH - QR) / 2.0;
    let qr_y = sheet_y + SHEET_PAD;
    let code = code_path(card.payload).map_or_else(String::new, |(modules, path)| {
        format!(
            r#"<svg x="{qr_x}" y="{qr_y}" width="{QR}" height="{QR}" viewBox="0 0 {modules} {modules}" shape-rendering="crispEdges"><path d="{path}" fill="{INK}"/></svg>"#
        )
    });
    let cx = half;
    let cy = qr_y + QR / 2.0;
    let plate = format!(
        r#"<rect x="{x}" y="{y}" width="{PLATE}" height="{PLATE}" rx="{PLATE_RADIUS}" fill="{PAPER}"/>"#,
        x = cx - PLATE / 2.0,
        y = cy - PLATE / 2.0,
    );
    // With a logo, the raster pass draws it (and its ring) over the plate.
    let disc = if logo {
        String::new()
    } else {
        format!(
            r#"<circle cx="{cx}" cy="{cy}" r="{radius}" fill="{tint}"/>
<text x="{cx}" y="{y}" text-anchor="middle" font-family="{SANS}" font-size="14" font-weight="700" fill="{PAPER}">{ticker}</text>"#,
            radius = LOGO / 2.0,
            tint = hex(card.network_tint),
            y = baseline(cy, 14.0),
            ticker = escape(card.network_ticker),
        )
    };

    // The account: identicon left, name and the whole address beside it.
    let id_top = qr_y + QR + IDENTITY_GAP;
    let text_height = NAME_LINE + NAME_ADDRESS_GAP + ADDRESS_LINE * 2.0;
    let text_room = QR - IDENTICON - IDENTITY_TEXT_GAP;
    let name = truncate(card.name, text_room, |t| measure(t, NAME_SIZE, 700, false));
    let text_width = measure(&name, NAME_SIZE, 700, false)
        .max(measure(card.lines.0, ADDRESS_SIZE, 400, true))
        .max(measure(card.lines.1, ADDRESS_SIZE, 400, true))
        .min(text_room);
    let id_x = r(cx - (IDENTICON + IDENTITY_TEXT_GAP + text_width) / 2.0);
    let id_y = id_top + (text_height - IDENTICON) / 2.0;
    // The identicon, from the same seed AS THE AVATARS NORMALISE IT, clipped
    // on a `<g>` in the card's own coordinates (a clip on the nested `<svg>`
    // is read in the artwork's 64-unit space and clips everything away).
    let identicon = identicon_svg_for(card.seed)
        .map(|svg| {
            svg.replacen(
                "<svg",
                &format!(r#"<svg x="{id_x}" y="{id_y}" width="{IDENTICON}" height="{IDENTICON}""#),
                1,
            )
        })
        .unwrap_or_default();
    let text_x = id_x + IDENTICON + IDENTITY_TEXT_GAP;
    let name_text = format!(
        r#"<text x="{text_x}" y="{y}" font-family="{SANS}" font-size="{NAME_SIZE}" font-weight="700" fill="{INK}">{name}</text>"#,
        y = baseline(id_top + NAME_LINE / 2.0, NAME_SIZE),
        name = escape(&name),
    );
    let address_top = id_top + NAME_LINE + NAME_ADDRESS_GAP;
    let address = [card.lines.0, card.lines.1]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            #[allow(clippy::cast_precision_loss, reason = "two lines")]
            let y = baseline(address_top + ADDRESS_LINE * (i as f32 + 0.5), ADDRESS_SIZE);
            format!(
                r#"<text x="{text_x}" y="{y}" font-family="{mono}" font-size="{ADDRESS_SIZE}" fill="{INK}" fill-opacity="0.5">{}</text>"#,
                escape(line)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let sheet_bottom = id_top + text_height + SHEET_PAD_BOTTOM;

    // The field closes over the foot in one curve that dips at the centre —
    // the WeChat card's direction — and the brand stands on the white.
    let edge = sheet_bottom + CURVE_GAP;
    let lowest = edge + CURVE_DEPTH;
    let height = lowest + FOOT;
    let field = format!(
        r#"<path d="M0,0 H{WIDTH} V{edge} Q{half},{control} 0,{edge} Z" fill="{FIELD}"/>"#,
        control = edge + CURVE_DEPTH * 2.0,
    );
    let wordmark_width = measure(card.wordmark, WORDMARK_SIZE, 700, false);
    let brand_x = r(half - (ICON + ICON_GAP + wordmark_width) / 2.0);
    let brand_centre = lowest + FOOT / 2.0;
    let icon = app_icon(brand_x, brand_centre - ICON / 2.0, ICON);
    let wordmark = format!(
        r#"<text x="{x}" y="{y}" font-family="{SANS}" font-size="{WORDMARK_SIZE}" font-weight="700" fill="{INK}">{text}</text>"#,
        x = brand_x + ICON + ICON_GAP,
        y = baseline(brand_centre, WORDMARK_SIZE),
        text = escape(card.wordmark),
    );

    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{height}" viewBox="0 0 {WIDTH} {height}">
<defs><clipPath id="identicon-clip"><circle cx="{icx}" cy="{icy}" r="{ir}"/></clipPath></defs>
<rect width="{WIDTH}" height="{height}" fill="{PAPER}"/>
{field}
{headline}{note}
<rect x="{sheet_x}" y="{sheet_y}" width="{SHEET_WIDTH}" height="{sheet_h}" rx="{SHEET_RADIUS}" fill="{PAPER}"/>
{code}
{plate}
{disc}
<g clip-path="url(#identicon-clip)">{identicon}</g>
{name_text}
{address}
{icon}
{wordmark}
</svg>"#,
        icx = id_x + IDENTICON / 2.0,
        icy = id_y + IDENTICON / 2.0,
        ir = IDENTICON / 2.0,
        sheet_h = sheet_bottom - sheet_y,
    );
    Composed {
        svg,
        height,
        logo_centre: (cx, cy),
    }
}

/// The card as an SVG document, measured by estimate and with the lettered
/// disc — pure, so a test can assert what it says and decode its code.
#[cfg(test)]
fn compose_svg(card: &ShareCard<'_>) -> String {
    compose(card, &estimate_width, false).svg
}

/// The faces the card is set in: the host's, plus the app's own Plus
/// Jakarta Sans (the web's face, bundled as TTF). Loaded once — a system
/// font scan is the slowest part of a save.
fn font_db() -> Arc<fontdb::Database> {
    static DB: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    Arc::clone(DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        for face in crate::theme::UI_FONT_FILES {
            db.load_font_data(face.to_vec());
        }
        Arc::new(db)
    }))
}

fn options() -> usvg::Options<'static> {
    let fallback = usvg::FontResolver::default_fallback_selector();
    let select_fallback: usvg::FallbackSelectionFn<'static> = Box::new(move |c, used, db| {
        let weight = used
            .first()
            .and_then(|id| db.face(*id))
            .map_or(fontdb::Weight::NORMAL, |face| face.weight);
        for family in CJK_SANS {
            let query = fontdb::Query {
                families: &[fontdb::Family::Name(family)],
                weight,
                ..fontdb::Query::default()
            };
            // No coverage check here (usvg keeps it private): a face offered
            // without the glyph joins `used`, and the shaper asks again —
            // so a Hangul syllable walks past PingFang to Apple SD Gothic Neo.
            if let Some(id) = db.query(&query)
                && !used.contains(&id)
            {
                return Some(id);
            }
        }
        fallback(c, used, db)
    });
    usvg::Options {
        fontdb: font_db(),
        font_resolver: usvg::FontResolver {
            select_font: usvg::FontResolver::default_font_selector(),
            select_fallback,
        },
        ..usvg::Options::default()
    }
}

/// The inked width of `text` as resvg itself lays it out, in the faces it
/// will be drawn in.
fn measure_text(
    options: &usvg::Options<'_>,
    text: &str,
    size: f32,
    weight: u16,
    mono: bool,
) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let family = if mono { mono_family() } else { SANS.to_owned() };
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="4000" height="200"><text x="0" y="100" font-family="{family}" font-size="{size}" font-weight="{weight}">{}</text></svg>"#,
        escape(text)
    );
    match usvg::Tree::from_str(&svg, options) {
        Ok(tree) if tree.root().has_children() => tree.root().abs_bounding_box().width(),
        _ => estimate_width(text, size, weight, mono),
    }
}

/// The logo at `size` px square: centre-cropped to a square (the web's
/// `preserveAspectRatio="xMidYMid slice"`), resampled, clipped to an
/// anti-aliased circle, premultiplied for tiny-skia. `None` when the bytes
/// are not an image — the disc stands in.
fn logo_pixmap(bytes: &[u8], size: u32) -> Option<tiny_skia::Pixmap> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    let side = width.min(height);
    if side == 0 {
        return None;
    }
    let square =
        image::imageops::crop_imm(&image, (width - side) / 2, (height - side) / 2, side, side)
            .to_image();
    let resized =
        image::imageops::resize(&square, size, size, image::imageops::FilterType::Lanczos3);
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    #[allow(
        clippy::cast_precision_loss,
        reason = "a logo is under a few hundred px"
    )]
    let radius = size as f32 / 2.0;
    let data = pixmap.data_mut();
    for (x, y, pixel) in resized.enumerate_pixels() {
        #[allow(
            clippy::cast_precision_loss,
            reason = "a logo is under a few hundred px"
        )]
        let (dx, dy) = (x as f32 + 0.5 - radius, y as f32 + 0.5 - radius);
        let coverage = (radius - dx.hypot(dy) + 0.5).clamp(0.0, 1.0);
        let alpha = f32::from(pixel[3]) / 255.0 * coverage;
        let at = ((y * size + x) * 4) as usize;
        for channel in 0..3 {
            data[at + channel] = (f32::from(pixel[channel]) * alpha).round() as u8;
        }
        data[at + 3] = (alpha * 255.0).round() as u8;
    }
    Some(pixmap)
}

/// The card as PNG bytes, at 2×.
///
/// `None` when the composition will not parse or the raster will not
/// allocate — the button then writes nothing rather than a broken file,
/// which is the same shape the web's `saveShareImage` has (it resolves
/// false).
#[must_use]
pub fn render_png(card: &ShareCard<'_>) -> Option<Vec<u8>> {
    let options = options();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "88 px"
    )]
    let logo = card
        .network_logo
        .and_then(|bytes| logo_pixmap(bytes, (LOGO * SCALE) as u32));
    let measure = |text: &str, size: f32, weight: u16, mono: bool| {
        measure_text(&options, text, size, weight, mono)
    };
    let composed = compose(card, &measure, logo.is_some());
    let tree = usvg::Tree::from_str(&composed.svg, &options).ok()?;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a card under a thousand px, times two"
    )]
    let (width, height) = (
        (WIDTH * SCALE).round() as u32,
        (composed.height * SCALE).round() as u32,
    );
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(SCALE, SCALE),
        &mut pixmap.as_mut(),
    );
    if let Some(logo) = logo {
        let (cx, cy) = composed.logo_centre;
        #[allow(clippy::cast_possible_truncation, reason = "inside the card")]
        let (x, y) = (
            ((cx - LOGO / 2.0) * SCALE).round() as i32,
            ((cy - LOGO / 2.0) * SCALE).round() as i32,
        );
        pixmap.draw_pixmap(
            x,
            y,
            logo.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::identity(),
            None,
        );
        // A hairline ring, so a logo drawn on transparency (Tempo's T)
        // still reads as a disc.
        if let Some(ring) =
            tiny_skia::PathBuilder::from_circle(cx * SCALE, cy * SCALE, (LOGO / 2.0 - 0.5) * SCALE)
        {
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(0x1a, 0x1a, 0x18, 20);
            paint.anti_alias = true;
            let stroke = tiny_skia::Stroke {
                width: SCALE,
                ..tiny_skia::Stroke::default()
            };
            pixmap.stroke_path(
                &ring,
                &paint,
                &stroke,
                tiny_skia::Transform::identity(),
                None,
            );
        }
    }
    pixmap.encode_png().ok()
}

/// The network's logo, fetched for a save on the system's routes for it
/// (`proxy::with_routes`). `None` on any failure — the card then wears the
/// lettered disc rather than waiting.
#[must_use]
pub fn fetch_logo(url: &str) -> Option<Vec<u8>> {
    let mut response =
        crate::executor::proxy::with_routes(url, LOGO_TIMEOUT, |agent| agent.get(url).call())
            .ok()?;
    let mut body = Vec::new();
    std::io::Read::read_to_end(&mut response.body_mut().as_reader(), &mut body).ok()?;
    Some(body)
}

/// The circular identicon for `seed`, exactly as the avatars draw it:
/// through `normalize_seed` first, then the core's generator. The one place
/// the card turns an address into a face, so it cannot drift from the screens
/// again by skipping the normalisation.
fn identicon_svg_for(seed: &str) -> Result<String, vela_core::CoreError> {
    vela_core::identicon::identicon_params(&vela_core::normalize_seed(seed))
        .map(|params| vela_core::identicon::assemble_svg_circular(&params))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADDRESS: &str = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c";

    /// The card's face is the screens' face for the same address, however
    /// the address is cased. The generator is case-sensitive, so the
    /// checksummed spelling drawn raw is a DIFFERENT creature — which is what
    /// the saved card showed until this was normalised.
    #[test]
    fn the_card_draws_the_same_identicon_as_the_avatars() {
        let on_screen = vela_core::identicon::identicon_params(&vela_core::normalize_seed(ADDRESS))
            .map(|params| vela_core::identicon::assemble_svg_circular(&params))
            .ok();
        assert!(on_screen.is_some());
        assert_eq!(identicon_svg_for(ADDRESS).ok(), on_screen);
        assert_eq!(
            identicon_svg_for(&ADDRESS.to_ascii_lowercase()).ok(),
            on_screen
        );
        let raw = vela_core::identicon::identicon_params(ADDRESS)
            .map(|params| vela_core::identicon::assemble_svg_circular(&params))
            .ok();
        assert_ne!(raw, on_screen, "the raw spelling is the wrong face");
    }

    fn card<'a>() -> ShareCard<'a> {
        ShareCard {
            headline: "扫码向我转账",
            payload: ADDRESS,
            name: "大表哥",
            lines: ("0x14fB1fB21751E29F7Ec", "48dC450017552E3D1eA5c"),
            network_note: "仅支持 Ethereum 网络付款",
            network_ticker: "ETH",
            network_tint: gpui::Hsla::from(gpui::rgb(0x627eea)),
            network_logo: None,
            seed: ADDRESS,
            wordmark: "Vela Wallet",
        }
    }

    /// The card carries the address twice — as text a person can read, and in
    /// the code a camera reads. Both have to be there, or the picture is
    /// either unscannable or uncheckable.
    #[test]
    fn the_card_says_the_address_and_encodes_it() {
        let svg = compose_svg(&card());
        assert!(svg.contains("0x14fB1fB21751E29F7Ec"), "no address in text");
        assert!(svg.contains("48dC450017552E3D1eA5c"));
        assert!(
            svg.contains("shape-rendering=\"crispEdges\""),
            "no code was drawn"
        );
        assert!(svg.contains("扫码向我转账"));
        assert!(svg.contains("仅支持 Ethereum 网络付款"));
    }

    /// The canonical icon, on the icon's own orange field — not a sailboat
    /// drawn here, not the UI accent — and the field dips at the centre.
    #[test]
    fn the_card_wears_the_app_icon_and_its_curve_dips() {
        let svg = compose_svg(&card());
        assert!(svg.contains(
            r##"<rect x="1" y="1" width="66" height="66" rx="18" ry="18" fill="#f46d50"/>"##
        ));
        assert!(svg.contains("M13,46L55,46C52,52,47,55,40,55L28,55C21,55,16,52,13,46Z"));
        assert!(!svg.contains("<?xml"), "the icon file's prolog leaked in");
        let field = svg
            .split("<path d=\"M0,0 H480 V")
            .nth(1)
            .unwrap_or_else(|| unreachable!("no field"));
        let edge: f32 = field
            .split(' ')
            .next()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        let control: f32 = field
            .split(" Q240,")
            .nth(1)
            .and_then(|rest| rest.split(' ').next())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        assert!(control > edge, "the curve must dip into the foot");
    }

    /// The code is level H: a plain address is 37 modules across (29 at M),
    /// which is what leaves the logo plate room.
    #[test]
    fn the_code_is_level_h() {
        let (modules, path) = code_path(ADDRESS).unwrap_or_else(|| unreachable!("no code"));
        assert_eq!(modules, 37);
        assert!(!path.is_empty());
        // Nothing to encode draws no code rather than an empty one.
        assert!(code_path("").is_none());
    }

    /// A long headline goes to two lines that each fit; a short one stays on one.
    #[test]
    fn the_headline_fits_the_card() {
        assert_eq!(
            fit_headline("扫码向我转账", &estimate_width),
            (32.0, vec!["扫码向我转账".to_owned()])
        );
        let (size, lines) =
            fit_headline("Отсканируйте, чтобы отправить мне крипто", &estimate_width);
        assert_eq!(lines.len(), 2);
        for line in lines {
            assert!(estimate_width(&line, size, 700, false) <= TEXT_WIDTH);
        }
    }

    /// A name too long for its row is cut; the address never is.
    #[test]
    fn a_long_name_is_cut_and_the_address_is_not() {
        let mut card = card();
        card.name = "An account name far too long to sit beside the code";
        let svg = compose_svg(&card);
        assert!(svg.contains("…</text>"));
        assert!(svg.contains("0x14fB1fB21751E29F7Ec"));
        assert!(svg.contains("48dC450017552E3D1eA5c"));
    }

    /// A page's own text cannot break the document it is drawn into.
    #[test]
    fn text_from_elsewhere_is_escaped() {
        let mut card = card();
        card.name = "</text><script>x</script>";
        let svg = compose_svg(&card);
        assert!(
            !svg.contains("<script>"),
            "unescaped markup reached the card"
        );
        assert!(svg.contains("&lt;"));
    }

    /// It rasterises, and what comes out is a PNG.
    #[test]
    fn the_card_renders_to_png_bytes() {
        let png = render_png(&card()).unwrap_or_else(|| unreachable!("the card did not rasterise"));
        assert_eq!(
            &png[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
            "not a PNG"
        );
        assert!(
            png.len() > 10_000,
            "suspiciously small: {} bytes",
            png.len()
        );
    }

    /// A logo that is not an image leaves the disc, not a hole.
    #[test]
    fn a_broken_logo_falls_back_to_the_disc() {
        assert!(logo_pixmap(b"not an image", 88).is_none());
    }

    /// Write the real export out so a person can look at it, for a spread of
    /// languages, networks and names — with the logos fetched from the live
    /// chain-data endpoint. Ignored by default: this is a picture, and a
    /// picture is reviewed by eye. `VELA_CARD_OUT=<dir>`.
    #[test]
    #[ignore = "writes PNGs for review; fetches live logos"]
    fn write_cards_for_review() {
        let dir = std::env::var("VELA_CARD_OUT").unwrap_or_else(|_| "/tmp".to_owned());
        let logos = "https://ethereum-data.getvela.app/chainlogos";
        let cases: [(&str, &str, &str, &str, u32, &str); 5] = [
            (
                "zh-ethereum",
                "扫码向我转账",
                "大表哥",
                "仅支持 Ethereum 网络付款",
                1,
                "ETH",
            ),
            (
                "en-gnosis-webp",
                "Scan to Send Me Crypto",
                "MultiTest",
                "Gnosis payments only",
                100,
                "XDA",
            ),
            (
                "ru-long-headline-bnb",
                "Отсканируйте, чтобы отправить мне крипто",
                "Основной кошелёк",
                "Только платежи в сети BNB Smart Chain",
                56,
                "BNB",
            ),
            (
                "de-long-name-tempo",
                "Scannen, um mir Krypto zu senden",
                "Gemeinsames Haushaltskonto der Familie",
                "Nur Zahlungen über Tempo",
                4217,
                "USD",
            ),
            (
                "zh-no-logo",
                "扫码向我转账",
                "大表哥",
                "仅支持 Ethereum 网络付款",
                0,
                "ETH",
            ),
        ];
        for (file, headline, name, note, chain, ticker) in cases {
            let logo = (chain != 0)
                .then(|| fetch_logo(&format!("{logos}/eip155-{chain}.png")))
                .flatten();
            if chain != 0 {
                assert!(logo.is_some(), "{file}: the logo did not fetch");
            }
            let card = ShareCard {
                headline,
                name,
                network_note: note,
                network_ticker: ticker,
                network_logo: logo.as_deref(),
                ..card()
            };
            let png = render_png(&card).unwrap_or_else(|| unreachable!("{file}: no png"));
            let path = format!("{dir}/{file}.png");
            std::fs::write(&path, &png).unwrap_or_else(|error| unreachable!("{error}"));
            println!("wrote {path}");
        }
    }
}
