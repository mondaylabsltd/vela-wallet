//! Emit the TypeScript mirrors of the multisig machines' Core ↔ Shell wire
//! types (F0.4): the one `MultisigOperation` vocabulary, its results, and
//! each machine's events and view.
//!
//! Its own target and directory (`app-web/vela-wallet/src/lib/multisig/generated`),
//! so a multisig wire change never churns another machine family's mirror.
//! Run through `node rust/scripts/gen-core-types.mjs multisig` (which adds
//! the `--check` gate), not directly.

use std::{env, error::Error, fs, path::PathBuf};

use ts_rs::{Config, TS};
use vela_core::app::multisig_accounts::{Event as AccountsEvent, MultisigAccountsView};
use vela_core::app::multisig_members::{Event as MembersEvent, MultisigMembersView};
use vela_core::app::multisig_proposals::{Event as ProposalsEvent, ProposalsView};
use vela_core::app::multisig_shell::{MultisigOperation, MultisigShellResult};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = match env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
            .join("../../../app-web/vela-wallet/src/lib/multisig/generated"),
    };
    fs::create_dir_all(&out_dir)?;
    let config = Config::new().with_out_dir(&out_dir);
    MultisigOperation::export_all(&config)?;
    MultisigShellResult::export_all(&config)?;
    AccountsEvent::export_all(&config)?;
    MultisigAccountsView::export_all(&config)?;
    ProposalsEvent::export_all(&config)?;
    ProposalsView::export_all(&config)?;
    MembersEvent::export_all(&config)?;
    MultisigMembersView::export_all(&config)?;
    println!("multisig bindings written to {}", out_dir.display());
    Ok(())
}
