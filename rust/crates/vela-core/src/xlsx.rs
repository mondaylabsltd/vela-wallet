//! An Excel workbook's first sheet, as a grid of cell strings — the one reader
//! every platform uses.
//!
//! A payroll file is one of the two ways a person brings a list of people into
//! the wallet, and the grid it becomes decides how much each of them is paid.
//! Until this module each platform read workbooks its own way: the web through
//! SheetJS, which renders each cell's DISPLAY format (a 1234.567 shown as
//! `0.00` arrived as `1234.57`); iOS and Android through hand-rolled readers of
//! the raw stored value; the desktop through calamine. The same file could pay
//! different amounts depending on the device that opened it. Now there is one
//! reading, and it is this one.
//!
//! ## What a cell becomes
//!
//! - **Text** — exactly what was typed (shared strings, inline strings, formula
//!   results); phonetic guides (`rPh`) are not part of it.
//! - **A number** — the value the workbook STORES, rounded to Excel's own 15
//!   significant digits (a sum that is `0.30000000000000004` in binary reads
//!   `0.3`), written as a plain decimal. Never the display format: a format
//!   rounds what a person sees, not what they meant to pay. Never exponent
//!   notation either: the amount cleaner keeps digits and `.`, so `1e-7` would
//!   become seventeen.
//!
//!   Fifteen digits is not a cap this reader imposes on token amounts: it is
//!   all an Excel NUMBER cell can hold. Excel cuts `1.123456789012345678` to
//!   `1.12345678901234` the moment it is typed, and digits past the fifteenth
//!   in the XML are binary noise no one entered. Significant digits, not
//!   decimals — one wei (`0.000000000000000001`) is one digit and arrives
//!   whole. An amount that needs more than fifteen digits is kept by a TEXT
//!   cell (a Text column, or a leading `'`), which arrives character for
//!   character, as a CSV does.
//! - **A date or time** (a number under a date/time format) — ISO
//!   `2024-01-31`, `2024-01-31 09:30:00`, or `09:30:00`. A date column must
//!   never look like an amount column, and a bare serial (`45322`) would.
//! - **TRUE / FALSE** for booleans; **nothing** for an error (`#N/A`) or a
//!   formula with no cached result.
//!
//! Every row comes back the same width — the sheet's used columns, first to
//! last — because `batch_import` settles the amount column per row SHAPE, and
//! a row that came back short would be a different shape. Rows with no content
//! are left out (the importer drops blank lines anyway).
//!
//! ## What it refuses
//!
//! Binary workbooks (`.xls`, `.xlsb`), encrypted ones, ZIP64, and anything
//! whose parts will not open. Refusal is an error, never an empty grid: an
//! empty grid would read as "a file with no recipients in it".

use std::collections::{BTreeMap, HashMap};

/// Why a workbook was not read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum XlsxError {
    #[error("not a ZIP archive")]
    NotZip,
    #[error("unsupported workbook: {0}")]
    Unsupported(&'static str),
    #[error("missing part {0}")]
    Missing(String),
    #[error("malformed workbook: {0}")]
    Malformed(&'static str),
}

/// A part larger than this once inflated is refused rather than allocated:
/// a recipient list is kilobytes, and a ZIP bomb is not a payroll.
const MAX_PART_BYTES: usize = 64 << 20;
/// The widest sheet read. A payroll is a handful of columns; a stray cell far
/// to the right would otherwise widen every row to it.
const MAX_COLUMNS: u32 = 512;

/// Whether `bytes` open like a ZIP archive — the container of `.xlsx`.
#[must_use]
pub fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
}

/// Whether `bytes` are an OLE compound file — a legacy `.xls` (or an
/// encrypted `.xlsx`). Neither is read; both deserve "unreadable" rather than
/// a text decoder's opinion of their bytes.
#[must_use]
pub fn is_compound_file(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1])
}

/// Whether a file NAME says workbook (`.xlsx`, `.xlsm`, `.xls`, `.xlsb`), in
/// any case. A file so named is read as one even when its bytes are not — a
/// broken workbook is unreadable, never a CSV of its garbage.
#[must_use]
pub fn is_workbook_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".xlsx", ".xlsm", ".xls", ".xlsb"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

/// The first worksheet of an `.xlsx` / `.xlsm` workbook as rows of cell text.
///
/// # Errors
///
/// [`XlsxError`] when the bytes are not a workbook this reads; see the module
/// docs for what is refused.
pub fn first_sheet_rows(bytes: &[u8]) -> Result<Vec<Vec<String>>, XlsxError> {
    let zip = Zip::open(bytes)?;

    let workbook_path = zip
        .text_opt("_rels/.rels")?
        .and_then(|rels| {
            relationships(&rels)
                .into_iter()
                .find(|r| r.kind.ends_with("/officeDocument"))
                .map(|r| resolve("", &r.target))
        })
        .unwrap_or_else(|| "xl/workbook.xml".to_owned());
    let workbook = zip.text(&workbook_path)?;
    let (first_sheet_id, date1904) = workbook_facts(&workbook)?;

    let base = workbook_path
        .rsplit_once('/')
        .map_or("", |(dir, _)| dir)
        .to_owned();
    let rels_path = rels_path_for(&workbook_path);
    let rels = relationships(&zip.text(&rels_path)?);
    let target = |id: Option<&str>, kind: &str| -> Option<String> {
        rels.iter()
            .find(|r| id.map_or_else(|| r.kind.ends_with(kind), |id| r.id == id))
            .map(|r| resolve(&base, &r.target))
    };

    let sheet_path =
        target(Some(&first_sheet_id), "").ok_or(XlsxError::Malformed("first sheet has no part"))?;
    if !sheet_path.ends_with(".xml") {
        return Err(XlsxError::Unsupported("binary worksheet (.xlsb)"));
    }

    let shared = match target(None, "/sharedStrings") {
        Some(path) => zip
            .text_opt(&path)?
            .map(|xml| shared_strings(&xml))
            .unwrap_or_default(),
        None => Vec::new(),
    };
    let styles = match target(None, "/styles") {
        Some(path) => zip
            .text_opt(&path)?
            .map(|xml| Styles::parse(&xml))
            .unwrap_or_default(),
        None => Styles::default(),
    };

    sheet_rows(&zip.text(&sheet_path)?, &shared, &styles, date1904)
}

// ---------------------------------------------------------------------------
// ZIP
// ---------------------------------------------------------------------------

struct Entry {
    method: u16,
    flags: u16,
    compressed: usize,
    local: usize,
}

struct Zip<'a> {
    bytes: &'a [u8],
    /// Part names, lowercased: OOXML part names compare case-insensitively.
    entries: HashMap<String, Entry>,
}

fn u16_at(b: &[u8], at: usize) -> Result<u16, XlsxError> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or(XlsxError::Malformed("truncated archive"))
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, XlsxError> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(XlsxError::Malformed("truncated archive"))
}

impl<'a> Zip<'a> {
    fn open(bytes: &'a [u8]) -> Result<Self, XlsxError> {
        if !is_zip(bytes) {
            return Err(if is_compound_file(bytes) {
                XlsxError::Unsupported("legacy or encrypted workbook")
            } else {
                XlsxError::NotZip
            });
        }
        // The end record: the last 22 bytes, plus at most a 64 KiB comment.
        let floor = bytes.len().saturating_sub(22 + 0xffff);
        let eocd = (floor..=bytes.len().saturating_sub(22))
            .rev()
            .find(|&at| bytes[at..].starts_with(&[0x50, 0x4b, 0x05, 0x06]))
            .ok_or(XlsxError::Malformed("no central directory"))?;
        let disk = u16_at(bytes, eocd + 4)?;
        let count = u16_at(bytes, eocd + 10)?;
        let mut at = u32_at(bytes, eocd + 16)?;
        if disk != 0 || count == 0xffff || at == 0xffff_ffff {
            return Err(XlsxError::Unsupported("multi-disk or ZIP64 archive"));
        }

        // Sizes come from the central directory, which always has them; writers
        // that stream set the data-descriptor flag and leave the local header's
        // sizes zero.
        let mut entries = HashMap::new();
        for _ in 0..count {
            let at_usize = at as usize;
            if u32_at(bytes, at_usize)? != 0x0201_4b50 {
                return Err(XlsxError::Malformed("broken central directory"));
            }
            let name_len = usize::from(u16_at(bytes, at_usize + 28)?);
            let extra_len = usize::from(u16_at(bytes, at_usize + 30)?);
            let comment_len = usize::from(u16_at(bytes, at_usize + 32)?);
            let name = bytes
                .get(at_usize + 46..at_usize + 46 + name_len)
                .ok_or(XlsxError::Malformed("truncated archive"))?;
            entries.insert(
                String::from_utf8_lossy(name)
                    .trim_start_matches('/')
                    .to_lowercase(),
                Entry {
                    flags: u16_at(bytes, at_usize + 8)?,
                    method: u16_at(bytes, at_usize + 10)?,
                    compressed: u32_at(bytes, at_usize + 20)? as usize,
                    local: u32_at(bytes, at_usize + 42)? as usize,
                },
            );
            at = u32::try_from(at_usize + 46 + name_len + extra_len + comment_len)
                .map_err(|_| XlsxError::Malformed("central directory overflow"))?;
        }
        Ok(Self { bytes, entries })
    }

    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, XlsxError> {
        let Some(entry) = self.entries.get(&path.to_lowercase()) else {
            return Ok(None);
        };
        if entry.flags & 1 != 0 {
            return Err(XlsxError::Unsupported("encrypted archive entry"));
        }
        if u32_at(self.bytes, entry.local)? != 0x0403_4b50 {
            return Err(XlsxError::Malformed("broken local header"));
        }
        let start = entry.local
            + 30
            + usize::from(u16_at(self.bytes, entry.local + 26)?)
            + usize::from(u16_at(self.bytes, entry.local + 28)?);
        let data = self
            .bytes
            .get(start..start + entry.compressed)
            .ok_or(XlsxError::Malformed("truncated archive"))?;
        match entry.method {
            0 if data.len() <= MAX_PART_BYTES => Ok(Some(data.to_vec())),
            0 => Err(XlsxError::Unsupported("part too large")),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(data, MAX_PART_BYTES)
                .map(Some)
                .map_err(|_| XlsxError::Malformed("part does not inflate")),
            _ => Err(XlsxError::Unsupported("compression method")),
        }
    }

    fn text_opt(&self, path: &str) -> Result<Option<String>, XlsxError> {
        let Some(bytes) = self.read(path)? else {
            return Ok(None);
        };
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
        String::from_utf8(bytes.to_vec())
            .map(Some)
            .map_err(|_| XlsxError::Malformed("part is not UTF-8"))
    }

    fn text(&self, path: &str) -> Result<String, XlsxError> {
        self.text_opt(path)?
            .ok_or_else(|| XlsxError::Missing(path.to_owned()))
    }
}

// ---------------------------------------------------------------------------
// XML — a forward-only scanner; SpreadsheetML needs no more
// ---------------------------------------------------------------------------

enum Event<'x> {
    /// A start tag: local name, its raw attribute text, and whether it closed itself.
    Open(&'x str, &'x str, bool),
    Close(&'x str),
    Text(&'x str),
}

fn local_name(qname: &str) -> &str {
    qname.rsplit_once(':').map_or(qname, |(_, local)| local)
}

fn scan<'x>(xml: &'x str, mut on: impl FnMut(Event<'x>)) {
    let mut at = 0;
    while at < xml.len() {
        let Some(lt) = xml[at..].find('<').map(|i| at + i) else {
            on(Event::Text(&xml[at..]));
            return;
        };
        if lt > at {
            on(Event::Text(&xml[at..lt]));
        }
        let rest = &xml[lt..];
        if rest.starts_with("<!--") {
            at = rest.find("-->").map_or(xml.len(), |i| lt + i + 3);
            continue;
        }
        if let Some(cdata) = rest.strip_prefix("<![CDATA[") {
            let end = cdata.find("]]>").unwrap_or(cdata.len());
            // CDATA is literal text: it must not be unescaped later.
            on(Event::Text(&cdata[..end]));
            at = lt + 9 + end + 3;
            continue;
        }
        if rest.starts_with("<?") || rest.starts_with("<!") {
            at = rest.find('>').map_or(xml.len(), |i| lt + i + 1);
            continue;
        }
        // Attribute values may hold `>`: step over quotes.
        let mut quote = None;
        let mut end = None;
        for (i, c) in rest.char_indices().skip(1) {
            match (quote, c) {
                (Some(q), c) if c == q => quote = None,
                (Some(_), _) => {}
                (None, '"' | '\'') => quote = Some(c),
                (None, '>') => {
                    end = Some(i);
                    break;
                }
                _ => {}
            }
        }
        let Some(end) = end else { return };
        let body = &rest[1..end];
        at = lt + end + 1;
        if let Some(name) = body.strip_prefix('/') {
            on(Event::Close(local_name(name.trim())));
            continue;
        }
        let (body, self_closing) = body.strip_suffix('/').map_or((body, false), |b| (b, true));
        let split = body.find(char::is_whitespace).unwrap_or(body.len());
        let name = local_name(&body[..split]);
        on(Event::Open(name, &body[split..], self_closing));
        if self_closing {
            on(Event::Close(name));
        }
    }
}

/// One attribute by local name (`r:id` answers to `id`), unescaped.
fn attr(attrs: &str, key: &str) -> Option<String> {
    let mut rest = attrs;
    while let Some(eq) = rest.find('=') {
        let name = local_name(rest[..eq].trim());
        let after = rest[eq + 1..].trim_start();
        let quote = after.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let close = after[1..].find(quote)?;
        if name == key {
            return Some(unescape(&after[1..=close]));
        }
        rest = &after[close + 2..];
    }
    None
}

/// XML entities, then OOXML's `_xHHHH_` escapes for characters XML cannot hold.
fn unescape(s: &str) -> String {
    if !s.contains('&') && !s.contains("_x") {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        let Some(semi) = tail.find(';').filter(|&i| i <= 10) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let entity = &tail[1..semi];
        let decoded = match entity {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .map(|hex| u32::from_str_radix(hex, 16))
                .or_else(|| entity.strip_prefix('#').map(str::parse::<u32>))
                .and_then(Result::ok)
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &tail[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    decode_ooxml_escapes(&out)
}

fn decode_ooxml_escapes(s: &str) -> String {
    if !s.contains("_x") {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("_x") {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let hex = tail
            .get(2..6)
            .filter(|h| h.bytes().all(|b| b.is_ascii_hexdigit()));
        match (hex, tail.as_bytes().get(6)) {
            (Some(hex), Some(b'_')) => {
                // A lone surrogate code has no `char`; it becomes U+FFFD.
                let code = u32::from_str_radix(hex, 16).unwrap_or(0xfffd);
                out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                rest = &tail[7..];
            }
            _ => {
                out.push_str("_x");
                rest = &tail[2..];
            }
        }
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------
// Package parts
// ---------------------------------------------------------------------------

struct Relationship {
    id: String,
    kind: String,
    target: String,
}

fn relationships(xml: &str) -> Vec<Relationship> {
    let mut out = Vec::new();
    scan(xml, |event| {
        if let Event::Open("Relationship", attrs, _) = event {
            if attr(attrs, "TargetMode").as_deref() == Some("External") {
                return;
            }
            if let (Some(id), Some(target)) = (attr(attrs, "Id"), attr(attrs, "Target")) {
                out.push(Relationship {
                    id,
                    kind: attr(attrs, "Type").unwrap_or_default(),
                    target,
                });
            }
        }
    });
    out
}

/// `xl/_rels/workbook.xml.rels` for `xl/workbook.xml`.
fn rels_path_for(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

/// A relationship target, resolved against the source part's directory.
fn resolve(base: &str, target: &str) -> String {
    let mut parts: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        base.split('/').filter(|p| !p.is_empty()).collect()
    };
    for piece in target.split('/') {
        match piece {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/")
}

/// The first sheet's relationship id, and whether dates count from 1904.
fn workbook_facts(xml: &str) -> Result<(String, bool), XlsxError> {
    let mut first = None;
    let mut date1904 = false;
    scan(xml, |event| match event {
        Event::Open("sheet", attrs, _) if first.is_none() => first = attr(attrs, "id"),
        Event::Open("workbookPr", attrs, _) => {
            date1904 = matches!(attr(attrs, "date1904").as_deref(), Some("1" | "true"));
        }
        _ => {}
    });
    first
        .map(|id| (id, date1904))
        .ok_or(XlsxError::Malformed("workbook lists no sheet"))
}

/// Text inside `<t>` elements, outside phonetic runs (`<rPh>`).
#[derive(Default)]
struct RunText {
    text: String,
    in_t: bool,
    in_rph: bool,
}

impl RunText {
    fn on(&mut self, event: &Event<'_>) {
        match *event {
            Event::Open("t", _, _) => self.in_t = true,
            Event::Close("t") => self.in_t = false,
            Event::Open("rPh", _, _) => self.in_rph = true,
            Event::Close("rPh") => self.in_rph = false,
            Event::Text(text) if self.in_t && !self.in_rph => self.text.push_str(&unescape(text)),
            _ => {}
        }
    }

    fn take(&mut self) -> String {
        std::mem::take(&mut self.text).replace("\r\n", "\n")
    }
}

fn shared_strings(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut run = RunText::default();
    let mut in_si = false;
    scan(xml, |event| match event {
        Event::Open("si", _, _) => {
            in_si = true;
            run = RunText::default();
        }
        Event::Close("si") => {
            in_si = false;
            out.push(run.take());
        }
        ref e if in_si => run.on(e),
        _ => {}
    });
    out
}

/// What the styles part says about dates: which cell formats are date formats.
#[derive(Default)]
struct Styles {
    /// `cellXfs` in order: each cell style's number format id.
    xf_formats: Vec<u32>,
    /// The workbook's own format codes by id.
    custom: HashMap<u32, String>,
}

impl Styles {
    fn parse(xml: &str) -> Self {
        let mut styles = Self::default();
        let mut in_cell_xfs = false;
        scan(xml, |event| match event {
            Event::Open("numFmt", attrs, _) => {
                if let (Some(id), Some(code)) = (attr(attrs, "numFmtId"), attr(attrs, "formatCode"))
                {
                    if let Ok(id) = id.parse() {
                        styles.custom.insert(id, code);
                    }
                }
            }
            Event::Open("cellXfs", _, self_closing) => in_cell_xfs = !self_closing,
            Event::Close("cellXfs") => in_cell_xfs = false,
            Event::Open("xf", attrs, _) if in_cell_xfs => {
                styles.xf_formats.push(
                    attr(attrs, "numFmtId")
                        .and_then(|id| id.parse().ok())
                        .unwrap_or(0),
                );
            }
            _ => {}
        });
        styles
    }

    fn is_date(&self, style: Option<usize>) -> bool {
        let Some(id) = style.and_then(|s| self.xf_formats.get(s)).copied() else {
            return false;
        };
        match self.custom.get(&id) {
            Some(code) => is_date_format(code),
            // The built-in date and time formats (ECMA-376 18.8.30, with the
            // East Asian ones Excel assigns to 27–36 and 50–58).
            None => matches!(id, 14..=22 | 27..=36 | 45..=47 | 50..=58 | 71..=81),
        }
    }
}

/// Whether a format code shows a date or a time: a `y m d h s` outside quotes,
/// escapes and brackets, or an elapsed-time bracket (`[h]`, `[mm]`).
fn is_date_format(code: &str) -> bool {
    let lower = code.to_lowercase().replace("general", "");
    let mut chars = lower.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                for q in chars.by_ref() {
                    if q == '"' {
                        break;
                    }
                }
            }
            '\\' | '_' | '*' => {
                chars.next();
            }
            '[' => {
                let inner: String = chars.by_ref().take_while(|&b| b != ']').collect();
                if !inner.is_empty() && inner.chars().all(|b| matches!(b, 'h' | 'm' | 's')) {
                    return true;
                }
            }
            'y' | 'm' | 'd' | 'h' | 's' => return true,
            _ => {}
        }
    }
    false
}

// ---------------------------------------------------------------------------
// The sheet
// ---------------------------------------------------------------------------

/// `B7` → (row 6, column 1), zero-based.
fn cell_ref(r: &str) -> Option<(u32, u32)> {
    let letters = r.bytes().take_while(u8::is_ascii_alphabetic).count();
    if letters == 0 || letters > 3 {
        return None;
    }
    let col = r[..letters].bytes().fold(0u32, |acc, b| {
        acc * 26 + u32::from(b.to_ascii_uppercase() - b'A' + 1)
    });
    let row: u32 = r[letters..].parse().ok()?;
    Some((row.checked_sub(1)?, col - 1))
}

#[derive(Default)]
struct Cell {
    kind: Option<String>,
    style: Option<usize>,
    value: String,
    run: RunText,
    in_v: bool,
    in_is: bool,
}

fn sheet_rows(
    xml: &str,
    shared: &[String],
    styles: &Styles,
    date1904: bool,
) -> Result<Vec<Vec<String>>, XlsxError> {
    let mut rows: BTreeMap<u32, BTreeMap<u32, String>> = BTreeMap::new();
    let mut row: u32 = 0;
    let mut next_row: u32 = 0;
    let mut col: u32 = 0;
    let mut cell: Option<Cell> = None;
    let mut failure = None;

    scan(xml, |event| match event {
        Event::Open("row", attrs, _) => {
            row = attr(attrs, "r")
                .and_then(|r| r.parse::<u32>().ok())
                .and_then(|r| r.checked_sub(1))
                .unwrap_or(next_row);
            next_row = row + 1;
            col = 0;
        }
        Event::Open("c", attrs, _) => {
            if let Some((r, c)) = attr(attrs, "r").as_deref().and_then(cell_ref) {
                row = r;
                col = c;
            }
            cell = Some(Cell {
                kind: attr(attrs, "t"),
                style: attr(attrs, "s").and_then(|s| s.parse().ok()),
                ..Cell::default()
            });
        }
        Event::Close("c") => {
            if let Some(done) = cell.take() {
                match cell_text(done, shared, styles, date1904) {
                    Ok(text) if !text.is_empty() => {
                        rows.entry(row).or_default().insert(col, text);
                    }
                    Ok(_) => {}
                    Err(e) => failure = Some(e),
                }
            }
            col += 1;
        }
        ref e => {
            if let Some(cell) = cell.as_mut() {
                match *e {
                    Event::Open("v", _, _) => cell.in_v = true,
                    Event::Close("v") => cell.in_v = false,
                    Event::Open("is", _, _) => cell.in_is = true,
                    Event::Close("is") => cell.in_is = false,
                    Event::Text(text) if cell.in_v => cell.value.push_str(text),
                    _ if cell.in_is => cell.run.on(e),
                    _ => {}
                }
            }
        }
    });
    if let Some(e) = failure {
        return Err(e);
    }

    let first = rows
        .values()
        .filter_map(|r| r.keys().next())
        .min()
        .copied()
        .unwrap_or(0);
    let last = rows
        .values()
        .filter_map(|r| r.keys().next_back())
        .max()
        .copied()
        .unwrap_or(0);
    let width = last - first + 1;
    if width > MAX_COLUMNS {
        return Err(XlsxError::Unsupported("sheet is too wide"));
    }
    Ok(rows
        .into_values()
        .map(|cells| {
            let mut out = vec![String::new(); width as usize];
            for (c, text) in cells {
                out[(c - first) as usize] = text;
            }
            out
        })
        .collect())
}

fn cell_text(
    mut cell: Cell,
    shared: &[String],
    styles: &Styles,
    date1904: bool,
) -> Result<String, XlsxError> {
    let raw = unescape(cell.value.trim());
    Ok(match cell.kind.as_deref() {
        Some("s") => {
            if raw.is_empty() {
                return Ok(String::new());
            }
            let index: usize = raw
                .parse()
                .map_err(|_| XlsxError::Malformed("shared string index"))?;
            shared
                .get(index)
                .cloned()
                .ok_or(XlsxError::Malformed("shared string index"))?
        }
        Some("inlineStr") => cell.run.take(),
        Some("str") => unescape(&cell.value).replace("\r\n", "\n"),
        Some("b") => match raw.as_str() {
            "1" | "true" => "TRUE".to_owned(),
            "" => String::new(),
            _ => "FALSE".to_owned(),
        },
        Some("e") => String::new(),
        Some("d") => iso_from_stored(&raw),
        _ => {
            if raw.is_empty() {
                return Ok(String::new());
            }
            let value: f64 = raw
                .parse()
                .map_err(|_| XlsxError::Malformed("number cell"))?;
            if styles.is_date(cell.style) {
                serial_to_iso(value, date1904).unwrap_or_else(|| number_text(value))
            } else {
                number_text(value)
            }
        }
    })
}

/// A stored number as a plain decimal at Excel's 15 significant digits.
fn number_text(value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }
    if value == 0.0 {
        return "0".to_owned();
    }
    let sci = format!("{value:.14e}"); // d.dddddddddddddde±x
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let negative = mantissa.starts_with('-');
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let point = exp + 1; // digits before the decimal point
    let body = if point <= 0 {
        format!("0.{}{digits}", "0".repeat(point.unsigned_abs() as usize))
    } else if point as usize >= digits.len() {
        format!("{digits}{}", "0".repeat(point as usize - digits.len()))
    } else {
        format!(
            "{}.{}",
            &digits[..point as usize],
            &digits[point as usize..]
        )
    };
    if negative {
        format!("-{body}")
    } else {
        body
    }
}

/// Days since 1970-01-01 → (year, month, day) — Howard Hinnant's `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// An Excel serial date as ISO text. `None` outside Excel's calendar.
fn serial_to_iso(serial: f64, date1904: bool) -> Option<String> {
    if !(0.0..2_958_466.0).contains(&serial) {
        return None;
    }
    let mut days = serial.floor() as i64;
    let mut seconds = ((serial - serial.floor()) * 86_400.0).round() as i64;
    if seconds == 86_400 {
        days += 1;
        seconds = 0;
    }
    let time = format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    );
    // 1970-01-01 is day 25 569 in the 1900 system and day 24 107 in 1904's.
    let date = if date1904 {
        let (y, m, d) = civil_from_days(days - 24_107);
        format!("{y:04}-{m:02}-{d:02}")
    } else if days == 0 {
        // Serial 0 is no day — a time on its own.
        return Some(time);
    } else if days == 60 {
        // Excel keeps Lotus 1-2-3's 29 February 1900, which never was.
        "1900-02-29".to_owned()
    } else {
        let (y, m, d) = civil_from_days(days - 25_569 + i64::from(days < 60));
        format!("{y:04}-{m:02}-{d:02}")
    };
    Some(if seconds == 0 {
        date
    } else {
        format!("{date} {time}")
    })
}

/// A `t="d"` cell stores ISO 8601 already: drop the `T`, a zero time, the zone.
fn iso_from_stored(raw: &str) -> String {
    let trimmed = raw.trim_end_matches('Z');
    match trimmed.split_once('T') {
        Some((date, time)) => {
            let time = time.split('.').next().unwrap_or(time);
            if time == "00:00:00" || time == "00:00" {
                date.to_owned()
            } else {
                format!("{date} {time}")
            }
        }
        None => trimmed.to_owned(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_the_stored_value_at_fifteen_digits() {
        assert_eq!(number_text(0.1 + 0.2), "0.3");
        assert_eq!(number_text(1234.5), "1234.5");
        assert_eq!(number_text(1234.567), "1234.567");
        assert_eq!(number_text(100.0), "100");
        assert_eq!(number_text(-2.5), "-2.5");
        assert_eq!(number_text(0.0), "0");
        assert_eq!(number_text(-0.0), "0");
        // Never exponent notation — the amount cleaner would read `1e-7` as 17.
        assert_eq!(number_text(1e-7), "0.0000001");
        assert_eq!(number_text(1e21), "1000000000000000000000");
        assert_eq!(number_text(123_456_789_012_345_680.0), "123456789012346000");
        // One wei is one significant digit: nothing is lost.
        assert_eq!(number_text(1e-18), "0.000000000000000001");
        assert_eq!(
            number_text(0.000_123_456_789_012_345),
            "0.000123456789012345"
        );
    }

    #[test]
    fn an_eighteen_decimal_amount_survives_as_text() {
        let shared = vec!["1.123456789012345678".to_owned()];
        let xml = r#"<sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="inlineStr"><is><t>0.000000000000000001</t></is></c></row></sheetData>"#;
        let rows =
            sheet_rows(xml, &shared, &Styles::default(), false).expect("a well-formed sheet");
        assert_eq!(
            rows,
            vec![vec![
                "1.123456789012345678".to_owned(),
                "0.000000000000000001".to_owned()
            ]]
        );
    }

    #[test]
    fn serials_become_iso_dates_on_both_calendars() {
        assert_eq!(
            serial_to_iso(45_322.0, false).as_deref(),
            Some("2024-01-31")
        );
        assert_eq!(
            serial_to_iso(45_322.395_833_333_3, false).as_deref(),
            Some("2024-01-31 09:30:00")
        );
        assert_eq!(serial_to_iso(1.0, false).as_deref(), Some("1900-01-01"));
        assert_eq!(serial_to_iso(59.0, false).as_deref(), Some("1900-02-28"));
        assert_eq!(serial_to_iso(60.0, false).as_deref(), Some("1900-02-29"));
        assert_eq!(serial_to_iso(61.0, false).as_deref(), Some("1900-03-01"));
        assert_eq!(serial_to_iso(0.5, false).as_deref(), Some("12:00:00"));
        assert_eq!(serial_to_iso(0.0, true).as_deref(), Some("1904-01-01"));
        assert_eq!(serial_to_iso(43_860.0, true).as_deref(), Some("2024-01-31"));
        assert_eq!(serial_to_iso(-1.0, false), None);
    }

    #[test]
    fn date_formats_are_told_from_number_formats() {
        for code in [
            "yyyy-mm-dd",
            "m/d/yy",
            "[$-409]d-mmm-yy;@",
            "h:mm AM/PM",
            "[h]:mm:ss",
            "yyyy\"年\"m\"月\"d\"日\"",
        ] {
            assert!(is_date_format(code), "{code}");
        }
        for code in [
            "General",
            "0.00",
            "#,##0.00_);(#,##0.00)",
            "\"$\"#,##0",
            "0.00E+00",
            "[Red]0.00",
            "@",
            "\"day\"0",
        ] {
            assert!(!is_date_format(code), "{code}");
        }
    }

    #[test]
    fn escapes_unwind() {
        assert_eq!(
            unescape("a &amp; b &lt;c&gt; &#20013;&#x6587;"),
            "a & b <c> 中文"
        );
        assert_eq!(unescape("line_x000D_break _x_ x"), "line\rbreak _x_ x");
        assert_eq!(unescape("&unknown; &"), "&unknown; &");
    }

    #[test]
    fn targets_resolve_against_their_part() {
        assert_eq!(
            resolve("xl", "worksheets/sheet1.xml"),
            "xl/worksheets/sheet1.xml"
        );
        assert_eq!(
            resolve("xl", "/xl/worksheets/sheet1.xml"),
            "xl/worksheets/sheet1.xml"
        );
        assert_eq!(
            resolve("xl/sub", "../worksheets/s.xml"),
            "xl/worksheets/s.xml"
        );
        assert_eq!(
            rels_path_for("xl/workbook.xml"),
            "xl/_rels/workbook.xml.rels"
        );
    }

    #[test]
    fn cell_refs_are_zero_based() {
        assert_eq!(cell_ref("A1"), Some((0, 0)));
        assert_eq!(cell_ref("B7"), Some((6, 1)));
        assert_eq!(cell_ref("AA10"), Some((9, 26)));
        assert_eq!(cell_ref("7"), None);
    }

    #[test]
    fn a_workbook_is_named_by_its_extension_in_any_case() {
        assert!(is_workbook_name("payroll.XLSX"));
        assert!(is_workbook_name("a.xlsm"));
        assert!(is_workbook_name("old.xls"));
        assert!(!is_workbook_name("payroll.csv"));
        assert!(!is_workbook_name("xlsx"));
    }

    #[test]
    fn binary_workbooks_are_refused_not_misread() {
        let ole = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0, 0];
        assert!(is_compound_file(&ole));
        assert_eq!(
            first_sheet_rows(&ole),
            Err(XlsxError::Unsupported("legacy or encrypted workbook"))
        );
        assert_eq!(
            first_sheet_rows(b"address,amount\n"),
            Err(XlsxError::NotZip)
        );
    }
}
