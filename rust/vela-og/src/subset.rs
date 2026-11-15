//! Cut a TrueType font down to the characters a set of texts uses — what
//! fontTools' `pyftsubset` does, for the one case the cards need.
//!
//! In: a `glyf`-outline font and some text. Out: a font with only those
//! characters' glyphs (plus `.notdef` and every component a composite glyph
//! draws from), renumbered densely, with the tables this crate's reader needs —
//! `head`, `hhea`, `maxp`, `OS/2`, `name`, `post` (v3), `cmap` (one format-12
//! subtable), `hmtx`, `loca` (long), `glyf`. Layout tables (`GPOS`, `GSUB`, …)
//! are dropped: the CJK faces this serves are set without kerning. The output
//! is a deterministic function of the input, so a committed subset can be
//! checked by re-running it.

use crate::font::Font;
use std::collections::{BTreeMap, BTreeSet};

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(at)?, *d.get(at + 1)?]))
}
fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *d.get(at)?,
        *d.get(at + 1)?,
        *d.get(at + 2)?,
        *d.get(at + 3)?,
    ]))
}

fn table<'a>(data: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    let n = u16_at(data, 4)? as usize;
    (0..n).find_map(|i| {
        let rec = 12 + 16 * i;
        if data.get(rec..rec + 4)? != tag {
            return None;
        }
        let off = u32_at(data, rec + 8)? as usize;
        let len = u32_at(data, rec + 12)? as usize;
        data.get(off..off.checked_add(len)?)
    })
}

/// Component glyph ids of a composite glyph, with the byte offset of each id.
fn components(glyph: &[u8]) -> Vec<(usize, u16)> {
    let mut out = Vec::new();
    if glyph.len() < 10 || (u16_at(glyph, 0).unwrap_or(0) as i16) >= 0 {
        return out;
    }
    let mut at = 10;
    loop {
        let (Some(flags), Some(gid)) = (u16_at(glyph, at), u16_at(glyph, at + 2)) else {
            return out;
        };
        out.push((at + 2, gid));
        at += 4 + if flags & 0x0001 != 0 { 4 } else { 2 };
        at += if flags & 0x0008 != 0 {
            2
        } else if flags & 0x0040 != 0 {
            4
        } else if flags & 0x0080 != 0 {
            8
        } else {
            0
        };
        if flags & 0x0020 == 0 {
            return out;
        }
    }
}

fn checksum(bytes: &[u8]) -> u32 {
    bytes.chunks(4).fold(0u32, |sum, c| {
        let mut w = [0u8; 4];
        w[..c.len()].copy_from_slice(c);
        sum.wrapping_add(u32::from_be_bytes(w))
    })
}

fn pad4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

/// The subset font, or `None` if `data` is not a `glyf` TrueType font.
pub fn subset(data: &[u8], text: &str) -> Option<Vec<u8>> {
    let font = Font::parse(data)?;
    let (head, hhea, maxp) = (
        table(data, b"head")?,
        table(data, b"hhea")?,
        table(data, b"maxp")?,
    );
    if head.len() < 54 || hhea.len() < 36 || maxp.len() < 6 {
        return None;
    }

    // Characters → original glyphs.
    let mut cmap: BTreeMap<u32, u16> = BTreeMap::new();
    for c in text.chars() {
        if let Some(g) = font.glyph_id(c) {
            cmap.insert(c as u32, g);
        }
    }

    // Close over composite components; .notdef always stays as glyph 0.
    let mut keep: BTreeSet<u16> = BTreeSet::from([0]);
    let mut stack: Vec<u16> = cmap.values().copied().collect();
    while let Some(g) = stack.pop() {
        if g >= font.num_glyphs() || !keep.insert(g) {
            continue;
        }
        for (_, child) in components(font.glyph_data(g).unwrap_or(&[])) {
            stack.push(child);
        }
    }
    let new_id: BTreeMap<u16, u16> = keep
        .iter()
        .enumerate()
        .map(|(i, &g)| (g, i as u16))
        .collect();
    let n = keep.len() as u16;

    // glyf + loca (long offsets), components renumbered.
    let mut glyf = Vec::new();
    let mut loca = Vec::with_capacity(4 * (n as usize + 1));
    let mut hmtx = Vec::with_capacity(4 * n as usize);
    for &old in &keep {
        loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
        let mut g = font.glyph_data(old).unwrap_or(&[]).to_vec();
        for (at, child) in components(&g.clone()) {
            let id = *new_id.get(&child)?;
            g[at..at + 2].copy_from_slice(&id.to_be_bytes());
        }
        glyf.extend_from_slice(&g);
        pad4(&mut glyf);
        hmtx.extend_from_slice(&(font.advance(old) as u16).to_be_bytes());
        hmtx.extend_from_slice(&font.lsb(old).to_be_bytes());
    }
    loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());

    // cmap: one (3, 10) format-12 subtable, consecutive runs grouped.
    let mut groups: Vec<(u32, u32, u32)> = Vec::new();
    for (&cp, old) in &cmap {
        let gid = u32::from(new_id[old]);
        match groups.last_mut() {
            // Extend the run only if both the code point and the glyph continue it.
            Some((start, end, start_gid))
                if *end + 1 == cp && *start_gid + (cp - *start) == gid =>
            {
                *end = cp;
            }
            _ => groups.push((cp, cp, gid)),
        }
    }
    let mut cmap_t = Vec::new();
    cmap_t.extend_from_slice(&[0, 0, 0, 1, 0, 3, 0, 10, 0, 0, 0, 12]);
    cmap_t.extend_from_slice(&12u16.to_be_bytes());
    cmap_t.extend_from_slice(&0u16.to_be_bytes());
    cmap_t.extend_from_slice(&((16 + 12 * groups.len()) as u32).to_be_bytes());
    cmap_t.extend_from_slice(&0u32.to_be_bytes());
    cmap_t.extend_from_slice(&(groups.len() as u32).to_be_bytes());
    for (start, end, gid) in &groups {
        cmap_t.extend_from_slice(&start.to_be_bytes());
        cmap_t.extend_from_slice(&end.to_be_bytes());
        cmap_t.extend_from_slice(&gid.to_be_bytes());
    }

    let mut head_t = head[..54].to_vec();
    head_t[8..12].copy_from_slice(&[0; 4]); // checkSumAdjustment, set below
    head_t[50..52].copy_from_slice(&1u16.to_be_bytes()); // long loca
    let mut hhea_t = hhea[..36].to_vec();
    hhea_t[34..36].copy_from_slice(&n.to_be_bytes());
    let mut maxp_t = maxp.to_vec();
    maxp_t[4..6].copy_from_slice(&n.to_be_bytes());
    let mut post_t =
        table(data, b"post").map_or_else(|| vec![0; 32], |p| p[..p.len().min(32)].to_vec());
    post_t.resize(32, 0);
    post_t[0..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());

    let mut tables: Vec<([u8; 4], Vec<u8>)> = vec![
        (*b"cmap", cmap_t),
        (*b"glyf", glyf),
        (*b"head", head_t),
        (*b"hhea", hhea_t),
        (*b"hmtx", hmtx),
        (*b"loca", loca),
        (*b"maxp", maxp_t),
        (*b"post", post_t),
    ];
    for tag in [b"OS/2", b"name"] {
        if let Some(t) = table(data, tag) {
            tables.push((*tag, t.to_vec()));
        }
    }
    tables.sort_by_key(|t| t.0);

    // Assemble: directory, then each table padded to four bytes.
    let count = tables.len() as u16;
    let entry_selector = 15 - count.leading_zeros() as u16;
    let search_range = (1u16 << entry_selector) * 16;
    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&(count * 16 - search_range).to_be_bytes());
    let mut offset = 12 + 16 * tables.len();
    let mut head_offset = 0;
    for (tag, body) in &tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum(body).to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        if tag == b"head" {
            head_offset = offset;
        }
        offset += body.len().div_ceil(4) * 4;
    }
    for (_, body) in &tables {
        out.extend_from_slice(body);
        pad4(&mut out);
    }
    let adjust = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
    out[head_offset + 8..head_offset + 12].copy_from_slice(&adjust.to_be_bytes());
    Some(out)
}
