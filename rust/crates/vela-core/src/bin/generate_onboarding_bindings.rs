//! Emit the TypeScript mirrors of the Core ↔ Shell wire types.
//!
//! The boundary is JSON, so nothing but this generator keeps the two sides
//! honest about ~40 variants. A drifted variant would fail at runtime, in a
//! branch the end-to-end suites do not reach (incompatible provider,
//! non-discoverable credential) — so the output is committed and gated, the
//! same way this repo treats every other generated artifact.
//!
//! Run through `node rust/scripts/gen-onboarding-types.mjs` (which adds the
//! `--check` drift gate), not directly.

use std::{env, error::Error, fs, path::PathBuf};

use ts_rs::{Config, TS};
use vela_core::app::create_wallet::{CreateView, Event as CreateWalletEvent};
use vela_core::app::login::{Event as LoginEvent, LoginView};
use vela_core::app::shell::{ShellOperation, ShellResult};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = match env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
            .join("../../../app-web/vela-wallet/src/lib/onboarding/generated"),
    };
    fs::create_dir_all(&out_dir)?;

    // `export_all` walks nested types, so these six roots cover the whole
    // surface: events in, operations out, results back, view models rendered.
    let config = Config::new().with_out_dir(&out_dir);
    CreateWalletEvent::export_all(&config)?;
    CreateView::export_all(&config)?;
    LoginEvent::export_all(&config)?;
    LoginView::export_all(&config)?;
    ShellOperation::export_all(&config)?;
    ShellResult::export_all(&config)?;

    println!("onboarding bindings written to {}", out_dir.display());
    Ok(())
}
