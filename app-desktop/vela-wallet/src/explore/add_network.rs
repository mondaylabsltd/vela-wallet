//! The add-network sheet a page opened (spec 100), in words.
//!
//! Every decision is the core's: whether the chain is known, whose words the
//! name and coin are, the verdict, whether Add acts. This file only picks the
//! corpus line each part of `NetView.dapp_add` is drawn with — Settings' own
//! words wherever they fit (`settingsModals.addNetwork.*`, `addToken.label*`)
//! — so the page draws it and a test can read it without a window.

use gpui::SharedString;
use vela_core::app::network_admin::{NetDappAddPhase, NetDappAddView};

use crate::loc::Loc;
use crate::settings::fixtures::Tone;

/// The sheet, resolved.
#[derive(Clone, Debug, PartialEq)]
pub struct AddNetworkSheet {
    pub title: SharedString,
    /// "{{host}} asks to add a network" — who is asking, from the transport.
    pub lead: SharedString,
    pub host: String,
    pub origin: String,
    /// Label and value, in order; a value the core does not know yet is left out.
    pub rows: Vec<(SharedString, SharedString)>,
    /// The name and coin are the site's, not Vela's catalog's.
    pub from_site: Option<SharedString>,
    pub pill: Option<(Tone, SharedString)>,
    /// A sentence under the verdict: the incompatible hint, a one-key-only
    /// chain, or why the page's RPC cannot be used.
    pub note: Option<SharedString>,
    /// Draw Settings' check list under the pill.
    pub checks: bool,
    /// "Add Network" — only where the core says it can act.
    pub add: Option<SharedString>,
    /// Retry after "unable to verify".
    pub retry: Option<SharedString>,
    /// The chain-setup tool, for a chain this wallet refuses.
    pub setup_tool: Option<SharedString>,
    /// The way out: Cancel while a decision is open, Done after a verdict.
    pub dismiss: SharedString,
}

#[must_use]
pub fn sheet(view: &NetDappAddView, loc: &Loc) -> AddNetworkSheet {
    let chain = view.chain_id.to_string();
    let name: SharedString = if view.name.is_empty() {
        loc.t_text("addToken.chainId", "chainId", &chain)
    } else {
        view.name.clone().into()
    };
    let mut rows = vec![
        (loc.t("addToken.labelName"), name),
        (loc.t("addToken.labelChainId"), chain.clone().into()),
    ];
    if !view.native_symbol.is_empty() {
        rows.push((
            loc.t("addToken.labelNativeToken"),
            view.native_symbol.clone().into(),
        ));
    }
    if let Some(host) = &view.rpc_host {
        rows.push((loc.t("addToken.labelRpcUrl"), host.clone().into()));
    }
    if let Some(host) = &view.explorer_host {
        rows.push((loc.t("addToken.labelExplorer"), host.clone().into()));
    }
    let cancel = loc.t("connect.browser.cancel");
    let done = loc.t("common.done");
    let mut out = AddNetworkSheet {
        title: loc.t("settingsModals.addNetwork.modalTitle"),
        lead: loc.t_text("connect.browser.addLead", "host", &view.host),
        host: view.host.clone(),
        origin: view.origin.clone(),
        rows,
        from_site: view.from_site.then(|| loc.t("connect.browser.addFromSite")),
        pill: None,
        note: None,
        checks: false,
        add: None,
        retry: None,
        setup_tool: None,
        dismiss: cancel.clone(),
    };
    match view.phase {
        NetDappAddPhase::Checking => {
            out.pill = Some((
                Tone::Neutral,
                loc.t("settingsModals.addNetwork.checkingCompatibility"),
            ));
        }
        NetDappAddPhase::Ready => {
            out.pill = Some((Tone::Ok, loc.t("settingsModals.addNetwork.compatible")));
            out.checks = true;
            // Spec 081 FR-009: works — and a wallet with more keys cannot be
            // made here. Both true, both said.
            out.note = view
                .compat
                .as_ref()
                .filter(|compat| !compat.multi_key_ready)
                .map(|_| loc.t("settingsModals.addNetwork.singleKeyOnly"));
            out.add = view
                .can_add
                .then(|| loc.t("settingsModals.addNetwork.addNetworkBtn"));
        }
        NetDappAddPhase::NotCompatible => {
            out.pill = Some((Tone::Error, loc.t("settingsModals.addNetwork.incompatible")));
            out.checks = true;
            out.note = Some(loc.t("settingsModals.addNetwork.incompatibleHint"));
            out.setup_tool = Some(loc.t("settingsModals.addNetwork.openChainSetupTool"));
            out.dismiss = done;
        }
        NetDappAddPhase::CheckFailed => {
            out.pill = Some((
                Tone::Warn,
                loc.t("settingsModals.addNetwork.unableToVerify"),
            ));
            out.retry = Some(loc.t("settingsModals.addNetwork.retry"));
        }
        NetDappAddPhase::WrongRpc => {
            let actual = view
                .reported_chain_id
                .map_or_else(String::new, |id| id.to_string());
            out.note = Some(loc.t_texts(
                "assets.rpcFixWrongChain",
                &[("actual", &actual), ("expected", &chain)],
            ));
            out.dismiss = done;
        }
        NetDappAddPhase::NoRpc => {
            out.note = Some(loc.t("componentsUi.browserStatus.reason.badRpc"));
            out.dismiss = done;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::network_admin::NetCompatibility;

    fn view(phase: NetDappAddPhase) -> NetDappAddView {
        NetDappAddView {
            tab: "t1".to_owned(),
            id: "7".to_owned(),
            origin: "https://app.example".to_owned(),
            host: "app.example".to_owned(),
            chain_id: 11_155_111,
            name: "Ethereum Sepolia".to_owned(),
            native_symbol: "ETH".to_owned(),
            rpc_host: Some("rpc.sepolia.example".to_owned()),
            explorer_host: Some("sepolia.etherscan.io".to_owned()),
            from_site: false,
            phase,
            reported_chain_id: None,
            compat: None,
            can_add: phase == NetDappAddPhase::Ready,
        }
    }

    fn loc() -> Loc {
        Loc::for_tag("en")
    }

    #[test]
    fn a_ready_chain_offers_add_in_settings_words() {
        let mut ready = view(NetDappAddPhase::Ready);
        ready.compat = Some(NetCompatibility {
            chain_id: ready.chain_id,
            compatible: true,
            multi_key_ready: true,
            contracts: Vec::new(),
            p256_available: Some(true),
            best_rpc_url: None,
            best_rpc_latency_ms: None,
            rpc_failure: None,
        });
        let sheet = sheet(&ready, &loc());
        assert_eq!(sheet.title.as_ref(), "Add Network");
        assert_eq!(sheet.lead.as_ref(), "app.example asks to add a network");
        assert_eq!(sheet.add.as_deref(), Some("Add Network"));
        assert_eq!(sheet.pill, Some((Tone::Ok, "Compatible".into())));
        assert_eq!(sheet.note, None, "a multi-key-ready chain needs no note");
        assert_eq!(sheet.dismiss.as_ref(), "Cancel");
        let labels: Vec<&str> = sheet.rows.iter().map(|(label, _)| label.as_ref()).collect();
        assert_eq!(
            labels,
            ["Name", "Chain ID", "Native Token", "RPC URL", "Explorer"]
        );
        assert_eq!(sheet.from_site, None);
    }

    #[test]
    fn verdicts_close_with_done_and_never_offer_add() {
        let incompatible = sheet(&view(NetDappAddPhase::NotCompatible), &loc());
        assert_eq!(incompatible.add, None);
        assert_eq!(incompatible.dismiss.as_ref(), "Done");
        assert!(incompatible.setup_tool.is_some());
        assert_eq!(
            incompatible.pill,
            Some((Tone::Error, "Incompatible".into()))
        );

        let mut wrong = view(NetDappAddPhase::WrongRpc);
        wrong.reported_chain_id = Some(100);
        wrong.from_site = true;
        let wrong = sheet(&wrong, &loc());
        assert_eq!(
            wrong.note.as_deref(),
            Some("That RPC serves a different network (chain 100, expected 11155111).")
        );
        assert_eq!(
            wrong.from_site.as_deref(),
            Some("Not in Vela’s network list — the name and coin are the site’s.")
        );

        let none = sheet(&view(NetDappAddPhase::NoRpc), &loc());
        assert_eq!(
            none.note.as_deref(),
            Some("The site gave no usable RPC for this network")
        );
        assert_eq!(none.add, None);
    }

    #[test]
    fn unable_to_verify_offers_retry_and_a_name_not_yet_known_is_the_chain() {
        let mut failed = view(NetDappAddPhase::CheckFailed);
        failed.name.clear();
        failed.native_symbol.clear();
        failed.rpc_host = None;
        failed.explorer_host = None;
        let sheet = sheet(&failed, &loc());
        assert_eq!(sheet.retry.as_deref(), Some("Retry"));
        assert_eq!(sheet.rows[0].1.as_ref(), "Chain 11155111");
        assert_eq!(sheet.rows.len(), 2, "only what the core knows");
    }
}
