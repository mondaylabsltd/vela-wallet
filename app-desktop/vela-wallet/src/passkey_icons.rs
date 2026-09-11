//! The passkey method icons — the desktop port of
//! `specs/038-first-run-parity/contracts/passkey-icons.json` (the founder's
//! set: Apple passkey, Windows passkey, Google Password Manager, Chrome on a
//! Mac, a FIDO2 key, a USB key).
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
    Apple,
    Windows,
    Google,
    ChromeMac,
    Fido2,
    Usb,
    /// lucide `laptop` — the device itself (stroke, 24-unit).
    Laptop,
    /// lucide `scan-line` — the camera that scans the code (stroke, 24-unit).
    Scan,
}

impl PasskeyIcon {
    /// The glyph for a method row (the founder's revision of #190): "this
    /// device" is THE DEVICE — a laptop, since this binary only runs on one —
    /// never a vendor's mark; "phone or tablet" is the scanner; the security
    /// key keeps the USB mark from the founder's set. The vendor marks stay
    /// in the contract for the key-row provider line, which knows the vault.
    pub fn for_method(method: KeyMethod) -> Self {
        match method {
            KeyMethod::Platform => Self::Laptop,
            KeyMethod::Hybrid => Self::Scan,
            KeyMethod::SecurityKey => Self::Usb,
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
    id: &'static str,
    /// The square viewBox side the contract draws in (16, or 80 for the key).
    view: f32,
    /// Markup with one `{}` where the fill goes, per element.
    elements: &'static [(Role, &'static str)],
}

fn def(icon: PasskeyIcon) -> IconDef {
    match icon {
        // Never reached: `svg_document` draws these before asking.
        PasskeyIcon::Laptop | PasskeyIcon::Scan => IconDef {
            id: "lucide",
            view: 24.,
            elements: &[],
        },
        PasskeyIcon::Apple => IconDef {
            id: "apple",
            view: 16.,
            elements: &[(
                Role::Ink,
                "<path d=\"M13.625 5.775c-.066.04-1.625.862-1.625 2.687.074 2.08 1.967 2.81 2 2.81-.033.04-.286.995-1.037 1.996-.595.862-1.256 1.732-2.26 1.732-.956 0-1.299-.575-2.4-.575-1.184 0-1.519.575-2.425.575-1.004 0-1.714-.916-2.343-1.77-.816-1.119-1.51-2.873-1.534-4.558-.016-.893.163-1.771.62-2.516.645-1.041 1.796-1.748 3.053-1.771.963-.03 1.82.629 2.408.629.563 0 1.616-.629 2.808-.629.514 0 1.885.148 2.735 1.39zM8 4.207c-.172-.816.301-1.631.742-2.151C9.306 1.426 10.196 1 10.963 1a2.952 2.952 0 0 1-.816 2.197c-.498.63-1.355 1.103-2.146 1.01z\" fill=\"{}\"/>",
            )],
        },
        PasskeyIcon::Windows => IconDef {
            id: "windows",
            view: 16.,
            elements: &[(
                Role::Ink,
                "<path d=\"M14 7.6H8.4V2H14v5.6zm-6.4 0V2H2v5.6h5.6zM2 8.4h5.6V14H2V8.4zm6.4 0V14H14V8.4H8.4z\" fill-rule=\"evenodd\" clip-rule=\"evenodd\" fill=\"{}\"/>",
            )],
        },
        PasskeyIcon::Google => IconDef {
            id: "google",
            view: 16.,
            elements: &[
                (
                    Role::Ink,
                    "<path d=\"M15 6.545h-3.818v1.91H15v-1.91zM4.5 11A3.51 3.51 0 0 1 1 7.5 3.51 3.51 0 0 1 4.5 4 3.51 3.51 0 0 1 8 7.5 3.51 3.51 0 0 1 4.5 11zm0-5.09c-.875 0-1.59.715-1.59 1.59 0 .875.715 1.59 1.59 1.59.875 0 1.59-.715 1.59-1.59 0-.875-.715-1.59-1.59-1.59z\" fill=\"{}\"/>",
                ),
                (
                    Role::Ink,
                    "<path d=\"M11.182 8.455v1.909h1.272v-.637c0-.35.287-.636.637-.636.35 0 .636.286.636.636v.637H15v-1.91h-3.818zM7.873 6.545h-2.1c.19.27.318.59.318.955 0 .366-.127.684-.318.955h2.1c.08-.303.127-.62.127-.955 0-.334-.048-.652-.127-.955z\" fill=\"{}\"/>",
                ),
                (
                    Role::Ink,
                    "<path d=\"M11.182 6.545h-3.31c.08.303.128.62.128.955 0 .334-.048.652-.127.955h3.309v-1.91z\" fill=\"{}\"/>",
                ),
            ],
        },
        PasskeyIcon::ChromeMac => IconDef {
            id: "chrome-mac",
            view: 16.,
            elements: &[
                (
                    Role::Ink,
                    "<path d=\"M14.667 12V3.333c0-.733-.6-1.333-1.334-1.333H2.667c-.734 0-1.334.6-1.334 1.333V12H0c0 .733.6 1.333 1.333 1.333h13.334c.733 0 1.333-.6 1.333-1.333h-1.333zm-12-8.667h10.666v8H2.667v-8z\" fill=\"{}\"/>",
                ),
                (
                    Role::Ink,
                    "<path d=\"M6.667 7.36c0 .367.133.68.393.94.26.26.573.393.94.393s.68-.133.94-.393c.26-.26.393-.573.393-.94s-.133-.68-.393-.94A1.284 1.284 0 0 0 8 6.027c-.367 0-.68.133-.94.393-.26.26-.393.573-.393.94zM8 9.36c.073 0 .14 0 .207-.007.066-.006.133-.02.2-.033l-.787 1.353a3.23 3.23 0 0 1-2.107-1.08 3.209 3.209 0 0 1-.853-2.226c0-.234.02-.46.067-.68.046-.22.113-.427.2-.627L6.26 8.367c.173.3.413.54.72.726.307.187.647.274 1.02.274V9.36zm0-4c-.447 0-.84.127-1.187.387-.346.26-.586.586-.72.986L5.307 5.38c.306-.413.693-.74 1.153-.987a3.213 3.213 0 0 1 1.527-.373c.553 0 1.053.12 1.52.36.466.24.846.567 1.153.973H7.993L8 5.36zm3.053.667a3.136 3.136 0 0 1 .287 1.333c0 .86-.28 1.6-.847 2.227a3.252 3.252 0 0 1-2.086 1.086L9.74 8.367c.087-.147.147-.3.193-.474.047-.173.074-.346.074-.526 0-.26-.047-.507-.14-.727a2.212 2.212 0 0 0-.38-.607h1.566v-.006z\" fill=\"{}\"/>",
                ),
            ],
        },
        PasskeyIcon::Fido2 => IconDef {
            id: "fido2",
            view: 16.,
            elements: &[
                (
                    Role::Ink,
                    "<path d=\"M8 6.667A1.333 1.333 0 1 0 8 4a1.333 1.333 0 0 0 0 2.667z\" fill=\"{}\"/>",
                ),
                (
                    Role::Ink,
                    "<path d=\"M8 1.333c-2.207 0-4 1.794-4 4v5.334C4 11.4 4.6 12 5.333 12H6v1.333c0 .734.6 1.334 1.333 1.334h1.334c.733 0 1.333-.6 1.333-1.334V12h.667C11.4 12 12 11.4 12 10.667V5.333c0-2.206-1.793-4-4-4zm2.667 9.334H5.333V5.333a2.666 2.666 0 1 1 5.334 0v5.334z\" fill=\"{}\"/>",
                ),
            ],
        },
        PasskeyIcon::Usb => IconDef {
            id: "usb",
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
        for icon in [
            PasskeyIcon::Apple,
            PasskeyIcon::Windows,
            PasskeyIcon::Google,
            PasskeyIcon::ChromeMac,
            PasskeyIcon::Fido2,
            PasskeyIcon::Usb,
        ] {
            let d = def(icon);
            let spec = icons
                .iter()
                .find(|i| i["id"] == d.id)
                .unwrap_or_else(|| panic!("{} is not in the contract", d.id));
            assert_eq!(
                spec["viewBox"][0].as_f64().unwrap() as f32,
                d.view,
                "{}",
                d.id
            );
            let elements = spec["elements"].as_array().unwrap();
            assert_eq!(elements.len(), d.elements.len(), "{}: element count", d.id);
            for (i, (role, markup)) in d.elements.iter().enumerate() {
                let el = &elements[i];
                let want_role = match el["role"].as_str().unwrap() {
                    "ink" => Role::Ink,
                    "muted" => Role::Muted,
                    _ => Role::Paper,
                };
                assert!(*role == want_role, "{}[{i}]: role", d.id);
                for key in ["d", "x", "y", "width", "height", "cx", "cy", "r"] {
                    if let Some(value) = el[key].as_str() {
                        assert!(
                            markup.contains(&format!("{key}=\"{value}\"")),
                            "{}[{i}]: {key} drifted from the contract",
                            d.id
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
            PasskeyIcon::Apple,
            PasskeyIcon::Usb,
            PasskeyIcon::ChromeMac,
            PasskeyIcon::Laptop,
            PasskeyIcon::Scan,
        ] {
            let image = crate::icons::rasterize(&svg_document(icon, &palette), 32);
            assert!(image.is_some(), "{icon:?} did not rasterise");
        }
    }
}
