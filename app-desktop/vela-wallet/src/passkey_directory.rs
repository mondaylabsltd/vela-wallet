//! Names for the authenticator models the compiled catalog cannot name.
//!
//! The catalog carries software passkey providers; hardware keys live in the
//! FIDO metadata service, hundreds of models deep, which is what the directory
//! service answers for. It is OUR service and stores nothing (founder,
//! 2026-08-26) — and the catalog still answers first, instantly and offline, so
//! this only ever runs for a key nothing on the machine could name.
//!
//! This platform is where it matters most: the desktop is the one client whose
//! keys are usually hardware.
//!
//! `vela_core::passkey` owns the contract — which AAGUIDs are worth asking
//! about, and what counts as an answer. This owns the transport and the memory.
//! A failure is remembered as "no answer" rather than retried on every frame.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use gpui::RenderImage;

use crate::raster::render_image_from_png;

/// A key list must never wait on a name.
const TIMEOUT: Duration = Duration::from_secs(6);

/// What the directory said, in this app's own vocabulary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Holder {
    pub name: String,
    pub icon_url: Option<String>,
}

#[derive(Default)]
pub struct PasskeyDirectory {
    /// `None` means asked with nothing to show for it.
    entries: HashMap<String, Option<Holder>>,
    marks: HashMap<String, Option<Arc<RenderImage>>>,
    asking: HashMap<String, ()>,
}

impl PasskeyDirectory {
    /// The settled answer for `aaguid`, or `None` while there is none.
    pub fn holder(&self, aaguid: &str, dark: bool) -> Option<&Holder> {
        self.entries.get(&key(aaguid, dark))?.as_ref()
    }

    /// The settled mark for `url` at `size`, or `None` while there is none.
    pub fn mark(&self, url: &str, size: u32) -> Option<Arc<RenderImage>> {
        self.marks.get(&format!("{url}@{size}"))?.clone()
    }

    /// Has anyone already asked this question? Callers use it to spawn once.
    pub fn claim(&mut self, id: String) -> bool {
        if self.entries.contains_key(&id) || self.asking.contains_key(&id) {
            return false;
        }
        self.asking.insert(id, ());
        true
    }

    pub fn settle(&mut self, aaguid: &str, dark: bool, holder: Option<Holder>) {
        let id = key(aaguid, dark);
        self.asking.remove(&id);
        self.entries.insert(id, holder);
    }

    pub fn claim_mark(&mut self, url: &str, size: u32) -> bool {
        let id = format!("{url}@{size}");
        if self.marks.contains_key(&id) || self.asking.contains_key(&id) {
            return false;
        }
        self.asking.insert(id, ());
        true
    }

    pub fn settle_mark(&mut self, url: &str, size: u32, image: Option<Arc<RenderImage>>) {
        let id = format!("{url}@{size}");
        self.asking.remove(&id);
        self.marks.insert(id, image);
    }
}

fn key(aaguid: &str, dark: bool) -> String {
    format!("{}|{dark}", aaguid.to_ascii_lowercase())
}

/// Which directory to ask (spec 038 #E4): the `aaguidDirectoryURL` of the
/// same service-endpoints object the index and the rates come from, or the
/// core's default until the settings name one. Read through, like the fiat
/// rates URL: a person who points it at our own node should not have to
/// relaunch for the next lookup to go there.
#[must_use]
pub fn directory_origin() -> String {
    crate::executor::storage::read_value(crate::executor::storage::KEY_SERVICE_ENDPOINTS)
        .ok()
        .flatten()
        .and_then(|value| {
            value
                .get("aaguidDirectoryURL")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| vela_core::passkey::AAGUID_DIRECTORY_ORIGIN.to_owned())
}

/// Ask the directory about `aaguid`. Blocking; callers run it off the UI thread.
#[must_use]
pub fn fetch_holder(aaguid: &str, dark: bool) -> Option<Holder> {
    let origin = directory_origin();
    let url = vela_core::passkey::directory_lookup_url_at(&origin, aaguid)?;
    let json = fetch(&url)?;
    let body = String::from_utf8(json).ok()?;
    vela_core::passkey::directory_entry_at(&origin, aaguid, &body, dark).map(|entry| Holder {
        name: entry.name,
        icon_url: entry.icon_url,
    })
}

/// Fetch and decode a directory mark. Blocking; callers run it off the UI thread.
///
/// The service serves both: a PNG decodes directly, an SVG goes through the same
/// rasterizer every other piece of core artwork uses.
#[must_use]
pub fn fetch_mark(url: &str, size: u32) -> Option<Arc<RenderImage>> {
    let bytes = fetch(url)?;
    if let Some(image) = render_image_from_png(&bytes) {
        return Some(image);
    }
    let svg = String::from_utf8(bytes).ok()?;
    let png = vela_core::rasterize_svg_png(&svg, size).ok()?;
    render_image_from_png(&png)
}

/// On the same route as every other request this shell makes (spec 038 Part
/// B): the system proxy first, then the environment's, then direct. A bare
/// agent here was the one lookup that ignored the person's proxy.
fn fetch(url: &str) -> Option<Vec<u8>> {
    let mut response = crate::executor::proxy::agent(TIMEOUT)
        .get(url)
        .call()
        .ok()?;
    let mut body = Vec::new();
    std::io::Read::read_to_end(&mut response.body_mut().as_reader(), &mut body).ok()?;
    Some(body)
}

#[cfg(test)]
mod origin_tests {
    use super::*;
    use crate::executor::storage;

    #[test]
    fn the_directory_is_the_endpoints_object_s_node_or_the_default() {
        storage::tests::with_temp_state("aaguid-directory", || {
            assert_eq!(
                directory_origin(),
                vela_core::passkey::AAGUID_DIRECTORY_ORIGIN
            );
            storage::write_value(
                storage::KEY_SERVICE_ENDPOINTS,
                serde_json::json!({ "aaguidDirectoryURL": "https://aaguid.getvela.app" }),
            )
            .expect("an endpoints write");
            assert_eq!(directory_origin(), "https://aaguid.getvela.app");
            // Blank is "not set", not "ask nowhere".
            storage::write_value(
                storage::KEY_SERVICE_ENDPOINTS,
                serde_json::json!({ "aaguidDirectoryURL": "  " }),
            )
            .expect("an endpoints write");
            assert_eq!(
                directory_origin(),
                vela_core::passkey::AAGUID_DIRECTORY_ORIGIN
            );
        });
    }
}
