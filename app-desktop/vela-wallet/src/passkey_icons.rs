//! The passkey method icons — the desktop port of
//! `specs/038-first-run-parity/contracts/passkey-icons.json`. The contract
//! carries the founder's whole set (Apple, Windows, Google, Chrome on a Mac,
//! FIDO2, USB); this shell ports only what it draws — the USB key — since
//! the founder's revision of #190 put a device glyph and a scanner on the
//! other two rows and no vendor mark anywhere.
//!
//! Separate from the lucide corpus in `icons.rs` on purpose: those are
//! 24-unit stroke glyphs tinted with one colour; these are 16- and 80-unit
//! FILLED marks with three roles — the ink, a muted body, and paper cut-outs
//! drawn OVER the ink (the USB key's slots) — so a single-colour substitution
//! would lose exactly the part that makes the key look like a key.
//!
//! "This device" resolves to the platform the app is RUNNING on
//! ([`PasskeyIcon::platform`]) — decided, never assumed (founder ruling on
//! #190). The contract test at the bottom re-reads the JSON and asserts every
//! element still matches.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{Hsla, RenderImage};
use vela_core::app::KeyMethod;

use crate::raster::empty_render_image;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PasskeyIcon {
    /// The founder's USB key mark (filled, 80-unit) — the security-key row.
    Usb,
    /// lucide `laptop` — the device itself (stroke, 24-unit).
    Laptop,
    /// lucide `scan-line` — the camera that scans the code (stroke, 24-unit).
    Scan,
    /// lucide `eye` — spec 075's Clear Signer row: a page you READ before you
    /// sign. Not a lock, not a key: the whole claim of that route is that you
    /// can see what is being signed. The same glyph the web client draws.
    Eye,
}

impl PasskeyIcon {
    /// The glyph for a method row (the founder's revision of #190): "this
    /// device" is THE DEVICE — a laptop, since this binary only runs on one —
    /// never a vendor's mark; "phone or tablet" is the scanner; the security
    /// key keeps the USB mark from the founder's set. The vendor marks stay
    /// in the contract, unported: nothing on this shell draws them.
    pub fn for_method(method: KeyMethod) -> Self {
        match method {
            KeyMethod::Platform => Self::Laptop,
            KeyMethod::Hybrid => Self::Scan,
            KeyMethod::SecurityKey => Self::Usb,
            KeyMethod::ClearSigner => Self::Eye,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Ink,
    Muted,
    Paper,
}

struct IconDef {
    /// The square viewBox side the contract draws in (80 for the key).
    view: f32,
    /// Markup with one `{}` where the fill goes, per element.
    elements: &'static [(Role, &'static str)],
}

fn def(icon: PasskeyIcon) -> IconDef {
    match icon {
        // Never reached: `svg_document` draws these before asking.
        PasskeyIcon::Laptop | PasskeyIcon::Scan | PasskeyIcon::Eye => IconDef {
            view: 24.,
            elements: &[],
        },
        PasskeyIcon::Usb => IconDef {
            view: 80.,
            elements: &[
                (
                    Role::Ink,
                    "<path d=\"M33 11C33 9.89543 33.8954 9 35 9H47C48.1046 9 49 9.89543 49 11V25H33V11Z\" fill=\"{}\"/>",
                ),
                (
                    Role::Paper,
                    "<rect x=\"35\" y=\"11\" width=\"2\" height=\"12\" fill=\"{}\"/>",
                ),
                (
                    Role::Paper,
                    "<rect x=\"40\" y=\"11\" width=\"2\" height=\"12\" fill=\"{}\"/>",
                ),
                (
                    Role::Paper,
                    "<rect x=\"45\" y=\"11\" width=\"2\" height=\"12\" fill=\"{}\"/>",
                ),
                (
                    Role::Muted,
                    "<path d=\"M31.7686 23H50.2272C51.2135 23 52.0737 23.7553 52.3154 24.8335C54.1052 32.8185 55.0001 40.7432 55.0001 48.6075C55.0001 56.5746 53.5305 64.5403 50.5915 72.5044C50.2573 73.41 49.4728 74 48.6029 74H33.3977C32.5275 74 31.7429 73.4096 31.4089 72.5036C28.4697 64.5306 27.0001 56.5398 27.0001 48.531C27.0001 40.6305 27.8939 32.7296 29.6816 24.8282C29.9249 23.7526 30.7841 23 31.7686 23Z\" fill=\"{}\"/>",
                ),
                (
                    Role::Ink,
                    "<path d=\"M41.0001 54C44.8661 54 48.0001 50.866 48.0001 47C48.0001 43.134 44.8661 40 41.0001 40C37.1341 40 34.0001 43.134 34.0001 47C34.0001 50.866 37.1341 54 41.0001 54Z\" fill=\"{}\"/>",
                ),
                (
                    Role::Paper,
                    "<path d=\"M41.0001 68C43.2092 68 45.0001 66.2091 45.0001 64C45.0001 61.7909 43.2092 60 41.0001 60C38.7909 60 37.0001 61.7909 37.0001 64C37.0001 66.2091 38.7909 68 41.0001 68Z\" fill=\"{}\"/>",
                ),
            ],
        },
    }
}

/// The three colours a mark is painted with.
#[derive(Clone, Copy)]
pub struct Palette {
    pub ink: Hsla,
    pub muted: Hsla,
    pub paper: Hsla,
}

fn hex(color: Hsla) -> (String, u32) {
    let rgba: gpui::Rgba = color.into();
    let r = (rgba.r * 255.).round() as u32;
    let g = (rgba.g * 255.).round() as u32;
    let b = (rgba.b * 255.).round() as u32;
    (format!("#{r:02x}{g:02x}{b:02x}"), (r << 16) | (g << 8) | b)
}

fn svg_document(icon: PasskeyIcon, palette: &Palette) -> String {
    // The two lucide glyphs: verbatim stroke defs, tinted with the ink.
    let lucide = match icon {
        PasskeyIcon::Laptop => Some(
            r##"<path d="M20 16V7a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v9m16 0H4m16 0 1.28 2.55a1 1 0 0 1-.9 1.45H3.62a1 1 0 0 1-.9-1.45L4 16"/>"##,
        ),
        PasskeyIcon::Scan => Some(
            r##"<path d="M3 7V5a2 2 0 0 1 2-2h2"/><path d="M17 3h2a2 2 0 0 1 2 2v2"/><path d="M21 17v2a2 2 0 0 1-2 2h-2"/><path d="M7 21H5a2 2 0 0 1-2-2v-2"/><path d="M7 12h10"/>"##,
        ),
        PasskeyIcon::Eye => Some(
            r##"<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/><circle cx="12" cy="12" r="3"/>"##,
        ),
        _ => None,
    };
    if let Some(inner) = lucide {
        let (ink, _) = hex(palette.ink);
        return format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="{ink}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{inner}</svg>"##
        );
    }
    let d = def(icon);
    let (ink, _) = hex(palette.ink);
    let (muted, _) = hex(palette.muted);
    let (paper, _) = hex(palette.paper);
    let mut inner = String::new();
    for (role, markup) in d.elements {
        let fill = match role {
            Role::Ink => &ink,
            Role::Muted => &muted,
            Role::Paper => &paper,
        };
        inner.push_str(&markup.replacen("{}", fill, 1));
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {v} {v}">{inner}</svg>"##,
        v = d.view
    )
}

#[derive(Default)]
pub struct PasskeyIconCache {
    map: HashMap<(PasskeyIcon, u32, u32, u32, u32), Arc<RenderImage>>,
}

impl PasskeyIconCache {
    /// The mark at `logical_px`, rasterised once per (icon, palette, size).
    pub fn image(
        &mut self,
        icon: PasskeyIcon,
        palette: Palette,
        logical_px: u32,
    ) -> Arc<RenderImage> {
        let size = logical_px * crate::icons::RASTER_SCALE;
        let key = (
            icon,
            hex(palette.ink).1,
            hex(palette.muted).1,
            hex(palette.paper).1,
            size,
        );
        if let Some(image) = self.map.get(&key) {
            return Arc::clone(image);
        }
        let image = crate::icons::rasterize(&svg_document(icon, &palette), size)
            .unwrap_or_else(empty_render_image);
        self.map.insert(key, Arc::clone(&image));
        image
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTRACT: &str =
        include_str!("../../../specs/038-first-run-parity/contracts/passkey-icons.json");

    /// SC-428: the desktop's marks are the contract's, element for element.
    #[test]
    fn every_mark_is_the_contract() {
        let json: serde_json::Value = serde_json::from_str(CONTRACT).expect("contract json");
        let icons = json["icons"].as_array().expect("icons");
        for (icon, id) in [(PasskeyIcon::Usb, "usb")] {
            let d = def(icon);
            let spec = icons
                .iter()
                .find(|i| i["id"] == id)
                .unwrap_or_else(|| panic!("{id} is not in the contract"));
            assert_eq!(spec["viewBox"][0].as_f64().unwrap() as f32, d.view, "{id}");
            let elements = spec["elements"].as_array().unwrap();
            assert_eq!(elements.len(), d.elements.len(), "{id}: element count");
            for (i, (role, markup)) in d.elements.iter().enumerate() {
                let el = &elements[i];
                let want_role = match el["role"].as_str().unwrap() {
                    "ink" => Role::Ink,
                    "muted" => Role::Muted,
                    _ => Role::Paper,
                };
                assert!(*role == want_role, "{id}[{i}]: role");
                for key in ["d", "x", "y", "width", "height", "cx", "cy", "r"] {
                    if let Some(value) = el[key].as_str() {
                        assert!(
                            markup.contains(&format!("{key}=\"{value}\"")),
                            "{id}[{i}]: {key} drifted from the contract"
                        );
                    }
                }
            }
        }
    }

    /// Every mark rasterises to something — an empty pixmap here would be an
    /// invisible row icon on a real screen.
    #[test]
    fn every_mark_rasterises() {
        let palette = Palette {
            ink: gpui::rgb(0x111111).into(),
            muted: gpui::rgb(0x888888).into(),
            paper: gpui::rgb(0xffffff).into(),
        };
        for icon in [
            PasskeyIcon::Usb,
            PasskeyIcon::Laptop,
            PasskeyIcon::Scan,
            PasskeyIcon::Eye,
        ] {
            let image = crate::icons::rasterize(&svg_document(icon, &palette), 32);
            assert!(image.is_some(), "{icon:?} did not rasterise");
        }
    }
}
