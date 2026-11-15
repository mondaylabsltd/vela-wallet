//! getvela.app's social-preview cards: one fixed layout drawn straight to a
//! 1200×630 PNG — no HTML, no SVG, no layout engine, no dependencies.
//!
//! The look is the site's own (static/getvela-app-preview2.png): a warm paper
//! background with a peach glow, the Vela mark and the tracked VELA WALLET
//! wordmark, the section as an accent pill, the page title large in ink, its
//! description in grey, the domain and a byline at the foot.
//!
//! Text is Inter (embedded, fonts/, SIL OFL 1.1), falling back per character
//! to fonts the host registers (`og_font`) — the CJK subsets the site cuts
//! with `og_subset`. A character no font has is refused, never drawn as a
//! stand-in. Build time only (app-web/getvela.app/scripts/og/), behind a plain
//! C ABI:
//!
//! ```text
//! og_alloc(len) -> ptr / og_free(ptr, len)   buffers the host writes into
//! og_font(ptr, len, weight) -> 1|0           register a fallback font (400 or 700)
//! og_fonts_clear()                           forget the fallback fonts
//! og_check(ptr, len) -> n                    0: drawable; else as og_render's refusal
//! og_render(ptr, len) -> n                   request: site \0 domain \0 section \0 title
//!                                            \0 description \0 meta (UTF-8)
//!                                            high bit clear, n > 0: n bytes of PNG
//!                                            high bit set: the characters no font has
//!                                            0: a malformed request
//! og_subset(font, len, text, len) -> n       n bytes of TrueType: `font` cut to `text`
//! og_result_ptr() -> ptr                     where the last result is
//! ```

pub mod font;
pub mod logo;
pub mod png;
pub mod raster;
pub mod subset;

use font::{Font, Point, Segment};
use raster::Coverage;
use std::cell::RefCell;

pub const WIDTH: usize = 1200;
pub const HEIGHT: usize = 630;
const PAD_X: f32 = 72.0;
const PAD_Y: f32 = 64.0;
const CONTENT_WIDTH: f32 = WIDTH as f32 - 2.0 * PAD_X;
const LOGO_SIZE: f32 = 56.0;

static INTER_REGULAR: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");
static INTER_BOLD: &[u8] = include_bytes!("../fonts/Inter-Bold.ttf");

/// The site's palette (tokens.css, light theme).
const PAPER: [f32; 3] = [250.0, 249.0, 245.0]; // --bg #faf9f5
const GLOW: [f32; 3] = [246.0, 190.0, 158.0]; // the peach of the preview image
const INK: Rgba = Rgba(35.0, 33.0, 28.0, 1.0); // --text #23211c
const SECONDARY: Rgba = Rgba(93.0, 90.0, 81.0, 1.0); // --text-secondary #5d5a51
const MUTED: Rgba = Rgba(125.0, 122.0, 111.0, 1.0); // --text-muted #7d7a6f
const ACCENT: Rgba = Rgba(232.0, 87.0, 42.0, 1.0); // --accent #e8572a

#[derive(Clone, Copy)]
struct Rgba(f32, f32, f32, f32);

/// What one card says. Empty fields are left out of the layout.
pub struct Card<'a> {
    pub site: &'a str,
    pub domain: &'a str,
    pub section: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub meta: &'a str,
}

/// Why a card was not drawn.
#[derive(Debug, PartialEq)]
pub enum Refusal {
    /// Characters no font has, each once, in order of appearance.
    Missing(String),
    /// The embedded fonts did not parse (a build bug).
    Fonts,
}

// ── Canvas ─────────────────────────────────────────────────────────────────

struct Canvas {
    px: Vec<[f32; 3]>,
}

impl Canvas {
    /// Paper, warmed by a soft peach glow rising from the bottom-right corner.
    fn paper() -> Canvas {
        let mut px = Vec::with_capacity(WIDTH * HEIGHT);
        let (cx, cy, rx, ry) = (1180.0f32, 700.0f32, 900.0f32, 640.0f32);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                let d = (dx * dx + dy * dy).sqrt().min(1.0);
                // Smoothstep falloff: no visible edge where the glow ends.
                let t = 1.0 - d;
                let a = 0.8 * t * t * (3.0 - 2.0 * t);
                px.push([0, 1, 2].map(|c| PAPER[c] + (GLOW[c] - PAPER[c]) * a));
            }
        }
        Canvas { px }
    }

    fn rgb(&self) -> Vec<u8> {
        self.px
            .iter()
            .flat_map(|p| p.map(|v| v.round().clamp(0.0, 255.0) as u8))
            .collect()
    }
}

/// Rasterize one shape into a buffer just big enough, then blend it in.
fn fill(canvas: &mut Canvas, segs: &[Segment], color: Rgba) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for seg in segs {
        for p in seg.points() {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
    }
    if x0 > x1 {
        return;
    }
    let (ox, oy) = (x0.floor() as i64 - 1, y0.floor() as i64 - 1);
    let w = (x1.ceil() as i64 - ox + 2).max(1) as usize;
    let h = (y1.ceil() as i64 - oy + 2).max(1) as usize;
    let mut cov = Coverage::new(w, h);
    let local: Vec<Segment> = segs
        .iter()
        .map(|seg| {
            seg.map(|p| Point {
                x: p.x - ox as f32,
                y: p.y - oy as f32,
            })
        })
        .collect();
    cov.fill(&local);
    let alpha = cov.finish();
    for j in 0..h {
        let cy = oy + j as i64;
        if cy < 0 || cy >= HEIGHT as i64 {
            continue;
        }
        for i in 0..w {
            let cx = ox + i as i64;
            if cx < 0 || cx >= WIDTH as i64 {
                continue;
            }
            let a = alpha[j * w + i] * color.3;
            if a <= 0.0 {
                continue;
            }
            let p = &mut canvas.px[cy as usize * WIDTH + cx as usize];
            p[0] += (color.0 - p[0]) * a;
            p[1] += (color.1 - p[1]) * a;
            p[2] += (color.2 - p[2]) * a;
        }
    }
}

/// The Vela mark, `size` px square, top-left at (x, y).
fn draw_logo(canvas: &mut Canvas, x: f32, y: f32, size: f32) {
    let s = size / logo::VIEWBOX;
    for (rgb, segs) in logo::shapes() {
        let placed: Vec<Segment> = segs
            .iter()
            .map(|seg| {
                seg.map(|p| Point {
                    x: x + p.x * s,
                    y: y + p.y * s,
                })
            })
            .collect();
        fill(canvas, &placed, Rgba(rgb[0], rgb[1], rgb[2], 1.0));
    }
}

// ── Text ───────────────────────────────────────────────────────────────────

/// Fonts for one weight: Inter first, then the registered fallbacks.
type Faces<'f> = [Font<'f>];

struct Glyph {
    face: usize,
    gid: u16,
}

/// A line of text at one size; each character in the first face that has it.
struct Run<'r, 'f> {
    faces: &'r Faces<'f>,
    size: f32,
    /// Extra advance after each glyph, in px (CSS letter-spacing).
    tracking: f32,
    glyphs: Vec<Glyph>,
}

impl<'r, 'f> Run<'r, 'f> {
    fn new(faces: &'r Faces<'f>, size: f32, text: &str) -> Run<'r, 'f> {
        let glyphs = text
            .chars()
            .map(|c| {
                faces
                    .iter()
                    .enumerate()
                    .find_map(|(face, f)| f.glyph_id(c).map(|gid| Glyph { face, gid }))
                    // `render` refuses uncovered text before any run is made.
                    .unwrap_or(Glyph { face: 0, gid: 0 })
            })
            .collect();
        Run {
            faces,
            size,
            tracking: 0.0,
            glyphs,
        }
    }

    fn tracked(mut self, em: f32) -> Self {
        self.tracking = em * self.size;
        self
    }

    fn scale(&self, face: usize) -> f32 {
        self.size / self.faces[face].units_per_em
    }

    /// Each glyph's x offset (px) and the run's width.
    fn layout(&self) -> (Vec<f32>, f32) {
        let mut pen = 0.0;
        let mut xs = Vec::with_capacity(self.glyphs.len());
        for (i, g) in self.glyphs.iter().enumerate() {
            if i > 0 {
                let prev = &self.glyphs[i - 1];
                if prev.face == g.face {
                    pen += self.faces[g.face].kerning(prev.gid, g.gid) * self.scale(g.face);
                }
                pen += self.tracking;
            }
            xs.push(pen);
            pen += self.faces[g.face].advance(g.gid) * self.scale(g.face);
        }
        (xs, pen)
    }

    fn width(&self) -> f32 {
        self.layout().1
    }

    /// The primary face's metrics set the line, whatever face a glyph is in.
    fn ascent(&self) -> f32 {
        self.faces[0].ascender * self.scale(0)
    }

    fn content_height(&self) -> f32 {
        (self.faces[0].ascender - self.faces[0].descender) * self.scale(0)
    }

    /// The baseline in a line box of `line_height` whose top is `top`.
    fn baseline(&self, top: f32, line_height: f32) -> f32 {
        top + (line_height - self.content_height()) / 2.0 + self.ascent()
    }

    fn draw(&self, canvas: &mut Canvas, x: f32, baseline: f32, color: Rgba) {
        let (xs, _) = self.layout();
        for (g, &gx) in self.glyphs.iter().zip(&xs) {
            let outline = self.faces[g.face].outline(g.gid);
            if outline.segments.is_empty() {
                continue;
            }
            let s = self.scale(g.face);
            let segs: Vec<Segment> = outline
                .segments
                .iter()
                .map(|seg| {
                    seg.map(|p| Point {
                        x: x + gx + p.x * s,
                        y: baseline - p.y * s,
                    })
                })
                .collect();
            fill(canvas, &segs, color);
        }
    }
}

/// Scripts set without spaces (Chinese, Japanese): a line may break between
/// any two characters. Korean is set with spaces and breaks between words,
/// as browsers do with `word-break: keep-all`.
fn breaks_anywhere(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x2FFF | 0x3000..=0x303F | 0x3040..=0x30FF | 0x3100..=0x31FF
        | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
        | 0xFF00..=0xFFEF | 0x20000..=0x3FFFF)
}

/// Never at the start of a line (closing punctuation, small kana, …).
fn no_break_before(c: char) -> bool {
    "，。、．・：；！？）」』】〕〉》’”…ー々ぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮヵヶ,.;:!?)]}%"
        .contains(c)
}

/// Never at the end of a line (opening punctuation).
fn no_break_after(c: char) -> bool {
    "（「『【〔〈《‘“([{".contains(c)
}

/// The text cut into pieces a line may break between: words, spaces, and
/// single characters of scripts written without spaces — with punctuation
/// glued to the side it belongs to.
fn break_units(text: &str) -> Vec<String> {
    let mut units: Vec<String> = Vec::new();
    for c in text.chars() {
        if c.is_whitespace() {
            if units.last().is_none_or(|u| u != " ") {
                units.push(" ".into());
            }
            continue;
        }
        let Some(u) = units.last_mut() else {
            units.push(c.to_string());
            continue;
        };
        let glue_back = u != " "
            && (no_break_before(c)
                || u.ends_with(no_break_after)
                || (!breaks_anywhere(c) && !u.chars().last().is_some_and(breaks_anywhere)));
        if glue_back {
            u.push(c);
        } else {
            units.push(c.to_string());
        }
    }
    units
}

/// Greedy line breaking at `max` px, at most `max_lines` lines; the last
/// line ends in "…" when the text did not fit (the flag says so). A unit
/// wider than a whole line is split by characters.
fn wrap(faces: &Faces, size: f32, text: &str, max: f32, max_lines: usize) -> (Vec<String>, bool) {
    let width = |s: &str| Run::new(faces, size, s).width();
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for unit in break_units(text) {
        if unit == " " {
            if !line.is_empty() {
                line.push(' ');
            }
            continue;
        }
        let candidate = format!("{line}{unit}");
        if width(candidate.trim_end()) <= max {
            line = candidate;
            continue;
        }
        if !line.trim_end().is_empty() {
            lines.push(line.trim_end().to_string());
        }
        line = String::new();
        for c in unit.chars() {
            line.push(c);
            if width(&line) > max && line.chars().count() > 1 {
                line.pop();
                lines.push(std::mem::take(&mut line));
                line.push(c);
            }
        }
    }
    if !line.trim_end().is_empty() {
        lines.push(line.trim_end().to_string());
    }
    let cut = lines.len() > max_lines;
    if cut {
        lines.truncate(max_lines);
        let last = lines.last_mut().expect("max_lines > 0");
        // End on a whole word where there are words: drop the one the cut
        // would split, before shortening further.
        if let Some(space) = last.trim_end().rfind(' ') {
            if width(&format!("{}…", last.trim_end())) > max {
                last.truncate(space);
            }
        }
        loop {
            let with = format!("{}…", last.trim_end());
            if width(&with) <= max || last.is_empty() {
                *last = with;
                break;
            }
            last.pop();
        }
    }
    (lines, cut)
}

// ── The card ───────────────────────────────────────────────────────────────

/// The characters of `text` that none of `faces` can draw.
fn missing(faces: &Faces, text: &str, into: &mut String) {
    for c in text.chars() {
        if !c.is_whitespace() && !faces.iter().any(|f| f.glyph_id(c).is_some()) && !into.contains(c)
        {
            into.push(c);
        }
    }
}

fn faces<'f>(
    primary: &'f [u8],
    weight: u16,
    fallbacks: &[(u16, &'f [u8])],
) -> Option<Vec<Font<'f>>> {
    let mut out = vec![Font::parse(primary)?];
    for &(w, bytes) in fallbacks {
        if w == weight {
            out.push(Font::parse(bytes)?);
        }
    }
    Some(out)
}

/// The section label as drawn: uppercase in scripts that have case.
fn section_label(card: &Card) -> String {
    card.section.to_uppercase()
}

/// `Ok(())` when every character the card draws is in some font.
pub fn check(card: &Card, fallbacks: &[(u16, &[u8])]) -> Result<(), Refusal> {
    let regular = faces(INTER_REGULAR, 400, fallbacks).ok_or(Refusal::Fonts)?;
    let bold = faces(INTER_BOLD, 700, fallbacks).ok_or(Refusal::Fonts)?;
    let section = section_label(card);
    let site = card.site.to_uppercase();
    let mut lacking = String::new();
    // A cut title or description also gets "…".
    for text in [site.as_str(), &section, card.title, card.domain, "…"] {
        missing(&bold, text, &mut lacking);
    }
    for text in [card.description, card.meta, "…"] {
        missing(&regular, text, &mut lacking);
    }
    if lacking.is_empty() {
        Ok(())
    } else {
        Err(Refusal::Missing(lacking))
    }
}

/// Draw one card; returns PNG bytes.
pub fn render(card: &Card, fallbacks: &[(u16, &[u8])]) -> Result<Vec<u8>, Refusal> {
    check(card, fallbacks)?;
    let regular = faces(INTER_REGULAR, 400, fallbacks).ok_or(Refusal::Fonts)?;
    let bold = faces(INTER_BOLD, 700, fallbacks).ok_or(Refusal::Fonts)?;
    let mut canvas = Canvas::paper();
    let right = WIDTH as f32 - PAD_X;

    // Top: the mark and the wordmark; the section as a pill on the right.
    draw_logo(&mut canvas, PAD_X, PAD_Y, LOGO_SIZE);
    let mid = PAD_Y + LOGO_SIZE / 2.0;
    let brand = Run::new(&bold, 22.0, &card.site.to_uppercase()).tracked(0.16);
    let brand_h = brand.content_height();
    brand.draw(
        &mut canvas,
        PAD_X + LOGO_SIZE + 18.0,
        brand.baseline(mid - brand_h / 2.0, brand_h),
        INK,
    );
    let section = section_label(card);
    if !section.is_empty() {
        // Letter-spacing is for capitals; CJK labels are set solid.
        let tracking = if section.chars().any(breaks_anywhere) {
            0.0
        } else {
            0.12
        };
        let label = Run::new(&bold, 17.0, &section).tracked(tracking);
        let (lw, lh) = (label.width(), label.content_height());
        let (pw, ph) = (lw + 40.0, 40.0);
        let pill = logo::rounded_rect([right - pw, mid - ph / 2.0, pw, ph, ph / 2.0]);
        fill(&mut canvas, &pill, Rgba(ACCENT.0, ACCENT.1, ACCENT.2, 0.1));
        label.draw(
            &mut canvas,
            right - pw + 20.0,
            label.baseline(mid - lh / 2.0, lh),
            ACCENT,
        );
    }

    // Foot: the domain on the left, the byline on the right.
    let domain = Run::new(&bold, 22.0, card.domain);
    let foot_h = domain.content_height();
    let foot_top = HEIGHT as f32 - PAD_Y - foot_h;
    domain.draw(&mut canvas, PAD_X, domain.baseline(foot_top, foot_h), INK);
    if !card.meta.is_empty() {
        let meta = Run::new(&regular, 22.0, card.meta);
        let w = meta.width();
        meta.draw(
            &mut canvas,
            right - w,
            meta.baseline(foot_top, foot_h),
            MUTED,
        );
    }

    // Middle: the title as large as it fits (three lines at most, four for
    // the longest), then the description in up to two.
    let (zone_top, zone_bottom) = (PAD_Y + LOGO_SIZE + 36.0, foot_top - 36.0);
    let zone = zone_bottom - zone_top;
    let mut title = (Vec::new(), 0.0);
    for (size, lines) in [(68.0, 2), (60.0, 3), (52.0, 3), (46.0, 4)] {
        let (l, cut) = wrap(&bold, size, card.title, CONTENT_WIDTH, lines);
        title = (l, size);
        if !cut {
            break;
        }
    }
    let (title_lines, title_size) = title;
    let title_lh = title_size * 1.16;
    let title_h = title_lh * title_lines.len() as f32;
    let (desc_size, gap) = (26.0, 24.0);
    let desc_lh = desc_size * 1.45;
    let room = ((zone - title_h - gap) / desc_lh).floor().clamp(0.0, 2.0) as usize;
    let desc_lines = if card.description.is_empty() || room == 0 {
        Vec::new()
    } else {
        wrap(&regular, desc_size, card.description, CONTENT_WIDTH, room).0
    };
    let block = title_h
        + if desc_lines.is_empty() {
            0.0
        } else {
            gap + desc_lh * desc_lines.len() as f32
        };
    // Optically centred: a touch above the geometric middle.
    let mut top = zone_top + ((zone - block) / 2.0 - 8.0).max(0.0);
    for line in &title_lines {
        let run = Run::new(&bold, title_size, line).tracked(-0.012);
        run.draw(&mut canvas, PAD_X, run.baseline(top, title_lh), INK);
        top += title_lh;
    }
    top += gap;
    for line in &desc_lines {
        let run = Run::new(&regular, desc_size, line);
        run.draw(&mut canvas, PAD_X, run.baseline(top, desc_lh), SECONDARY);
        top += desc_lh;
    }

    Ok(png::encode_rgb(WIDTH as u32, HEIGHT as u32, &canvas.rgb()))
}

/// Parse the C-ABI request: six NUL-separated UTF-8 fields.
pub fn parse_request(bytes: &[u8]) -> Option<Card<'_>> {
    let text = std::str::from_utf8(bytes).ok()?;
    let fields: Vec<&str> = text.split('\0').collect();
    let [site, domain, section, title, description, meta] = fields.as_slice() else {
        return None;
    };
    Some(Card {
        site,
        domain,
        section,
        title,
        description,
        meta,
    })
}

// ── C ABI ──────────────────────────────────────────────────────────────────

thread_local! {
    static RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static FALLBACKS: RefCell<Vec<(u16, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
}

#[no_mangle]
pub extern "C" fn og_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// # Safety
/// `ptr`/`len` must come from one `og_alloc` call.
#[no_mangle]
pub unsafe extern "C" fn og_free(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len));
}

#[no_mangle]
pub extern "C" fn og_result_ptr() -> *const u8 {
    RESULT.with(|r| r.borrow().as_ptr())
}

/// # Safety
/// `ptr` must point at `len` initialised bytes.
#[no_mangle]
pub unsafe extern "C" fn og_font(ptr: *const u8, len: usize, weight: u32) -> u32 {
    let bytes = std::slice::from_raw_parts(ptr, len).to_vec();
    if Font::parse(&bytes).is_none() || !matches!(weight, 400 | 700) {
        return 0;
    }
    FALLBACKS.with(|f| f.borrow_mut().push((weight as u16, bytes)));
    1
}

#[no_mangle]
pub extern "C" fn og_fonts_clear() {
    FALLBACKS.with(|f| f.borrow_mut().clear());
}

fn with_fallbacks<T>(f: impl FnOnce(&[(u16, &[u8])]) -> T) -> T {
    FALLBACKS.with(|cell| {
        let owned = cell.borrow();
        let borrowed: Vec<(u16, &[u8])> = owned.iter().map(|(w, b)| (*w, b.as_slice())).collect();
        f(&borrowed)
    })
}

fn respond(outcome: Option<Result<Vec<u8>, Refusal>>) -> u32 {
    RESULT.with(|r| {
        let mut r = r.borrow_mut();
        match outcome {
            Some(Ok(bytes)) => {
                *r = bytes;
                r.len() as u32
            }
            Some(Err(Refusal::Missing(chars))) => {
                *r = chars.into_bytes();
                0x8000_0000 | r.len() as u32
            }
            Some(Err(Refusal::Fonts)) | None => {
                r.clear();
                0
            }
        }
    })
}

/// # Safety
/// `ptr` must point at `len` initialised bytes.
#[no_mangle]
pub unsafe extern "C" fn og_render(ptr: *const u8, len: usize) -> u32 {
    let request = std::slice::from_raw_parts(ptr, len);
    respond(parse_request(request).map(|card| with_fallbacks(|fb| render(&card, fb))))
}

/// # Safety
/// `ptr` must point at `len` initialised bytes.
#[no_mangle]
pub unsafe extern "C" fn og_check(ptr: *const u8, len: usize) -> u32 {
    let request = std::slice::from_raw_parts(ptr, len);
    match parse_request(request).map(|card| with_fallbacks(|fb| check(&card, fb))) {
        Some(Ok(())) => {
            RESULT.with(|r| r.borrow_mut().clear());
            0
        }
        Some(Err(Refusal::Missing(chars))) => respond(Some(Err(Refusal::Missing(chars)))),
        Some(Err(Refusal::Fonts)) | None => 0x8000_0000,
    }
}

/// # Safety
/// `font`/`text` must point at `font_len`/`text_len` initialised bytes.
#[no_mangle]
pub unsafe extern "C" fn og_subset(
    font: *const u8,
    font_len: usize,
    text: *const u8,
    text_len: usize,
) -> u32 {
    let font = std::slice::from_raw_parts(font, font_len);
    let text = std::str::from_utf8(std::slice::from_raw_parts(text, text_len)).ok();
    respond(Some(
        text.and_then(|t| subset::subset(font, t))
            .ok_or(Refusal::Fonts),
    ))
}

#[cfg(test)]
mod tests;
