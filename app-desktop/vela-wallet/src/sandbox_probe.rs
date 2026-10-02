//! `VELA_SANDBOX_PROBE=1`: a developer build's report, once at startup, on
//! what the macOS App Sandbox lets this process do (spec 095) — the facts a
//! person cannot see on screen: where the state file really is and whether it
//! takes a write, where save panels open, whether HID enumeration and the
//! system proxy lookup work, whether an HTTPS request leaves. One line each
//! on stderr (`open --stderr <file>` keeps them for a bundle).
//!
//! Developer builds only, like every switch (`dev_env`): the store build
//! compiles the read out, so this never runs there.

/// Run the probe on a thread of its own when the switch is on.
pub fn run_if_asked() {
    if crate::dev_env::flag!("VELA_SANDBOX_PROBE") {
        std::thread::spawn(report);
    }
}

fn report() {
    let line = |what: &str, outcome: String| {
        eprintln!("[vela-wallet] sandbox probe: {what}: {outcome}");
    };
    line(
        "HOME",
        std::env::var("HOME").unwrap_or_else(|_| "(unset)".to_owned()),
    );
    line(
        "state file",
        match crate::executor::storage::path() {
            Ok(path) => path.display().to_string(),
            Err(error) => format!("ERROR {error:?}"),
        },
    );
    let stamp = serde_json::json!(std::process::id());
    let roundtrip = crate::executor::storage::write_value("vela.sandboxProbe", stamp.clone())
        .and_then(|()| crate::executor::storage::read_value("vela.sandboxProbe"));
    line(
        "state write+read",
        match roundtrip {
            Ok(Some(value)) if value == stamp => "ok".to_owned(),
            Ok(other) => format!("MISMATCH {other:?}"),
            Err(error) => format!("ERROR {error:?}"),
        },
    );
    let _ = crate::executor::storage::remove_value("vela.sandboxProbe");
    line(
        "save panels open in",
        crate::executor::storage::save_panel_dir()
            .display()
            .to_string(),
    );
    line(
        "HID devices listed",
        match crate::ctap::usb::hid_device_count() {
            Ok(count) => count.to_string(),
            Err(error) => format!("ERROR {error}"),
        },
    );
    line(
        "proxy for https://getvela.app/",
        match crate::executor::proxy::routes_for("https://getvela.app/") {
            Ok(routes) => format!("{routes:?}"),
            Err(error) => format!("ERROR {error:?}"),
        },
    );
    line(
        "HTTPS GET https://getvela.app/",
        match ureq::get("https://getvela.app/").call() {
            Ok(response) => response.status().to_string(),
            Err(error) => format!("ERROR {error}"),
        },
    );
}
