//! Signing — the desktop third column's request (spec 022).
//!
//! `fixtures` is the canon (data-model.md §3, all 33 CS scenarios),
//! `components` the universal block renderer. A scenario is a header, an
//! ORDERED list of blocks and a fixed footer; nothing in the renderer knows
//! what "a swap" is, which is what makes the six-rung ERC-7730 degradation
//! ladder structural rather than a fork per case.

pub mod components;
pub mod fixtures;
pub mod live;

use gpui::SharedString;

use crate::loc::Loc;

/// Semantic weight. `Accent` is the intent sentence; the rest colour warnings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Neutral,
    Accent,
    Success,
    Caution,
    Danger,
}

/// Every signing string, resolved once per locale. Roughly 95% of these keys
/// predate spec 022: the shipping React Native sheet already had them, and
/// reusing them is what keeps one wallet saying one thing about a transaction.
#[allow(
    dead_code,
    reason = "the catalogue fills every field; the desktop panel (DCS1–8) renders a subset"
)]
pub struct SigningStrings {
    pub panel_title: SharedString,
    pub signing_account: SharedString,
    pub advanced_toggle: SharedString,
    pub slide_to_confirm: SharedString,
    pub confirm_send: SharedString,
    pub confirm_swap: SharedString,
    pub confirm_deposit: SharedString,
    pub confirm_withdraw: SharedString,
    pub confirm_plain: SharedString,
    pub sign_label: SharedString,
    pub intent_send: SharedString,
    pub intent_approve: SharedString,
    pub intent_approve_all: SharedString,
    pub intent_revoke: SharedString,
    pub intent_swap: SharedString,
    pub intent_deposit: SharedString,
    pub intent_withdraw: SharedString,
    pub intent_transfer_nft: SharedString,
    pub intent_contract_call: SharedString,
    pub intent_batch: SharedString,
    pub intent_blind: SharedString,
    pub intent_sign_in: SharedString,
    pub intent_message: SharedString,
    pub intent_typed_data: SharedString,
    pub intent_permit: SharedString,
    pub intent_deploy: SharedString,
    pub intent_safe: SharedString,
    pub label_recipient: SharedString,
    pub label_spender: SharedString,
    pub label_operator: SharedString,
    pub label_collection: SharedString,
    pub label_interacting: SharedString,
    pub label_from: SharedString,
    pub label_amount: SharedString,
    pub label_deadline: SharedString,
    pub label_min_received: SharedString,
    pub label_pay: SharedString,
    pub label_siwe_site: SharedString,
    pub label_siwe_origin: SharedString,
    pub label_siwe_statement: SharedString,
    pub label_typed_domain: SharedString,
    pub label_type: SharedString,
    pub label_signing_for: SharedString,
    pub label_spending_cap: SharedString,
    pub label_expires: SharedString,
    pub label_resulting_total: SharedString,
    pub label_bytecode: SharedString,
    pub label_predicted_address: SharedString,
    pub label_deposit_asset: SharedString,
    pub label_shares_received: SharedString,
    pub tag_contact: SharedString,
    pub tag_wallet: SharedString,
    pub tag_contract: SharedString,
    pub tag_verified: SharedString,
    pub tag_unverified: SharedString,
    pub tag_first_time: SharedString,
    pub chip_requested: SharedString,
    pub chip_balance: SharedString,
    pub chip_custom: SharedString,
    pub chip_revoke: SharedString,
    pub chip_revoke_access: SharedString,
    pub chip_grant_all: SharedString,
    pub value_revoke: SharedString,
    pub value_unlimited: SharedString,
    pub value_all_nfts: SharedString,
    pub unlimited_disabled: SharedString,
    /// What a typed cap that is not a number gets told.
    pub invalid_amount: SharedString,
    pub choose_prompt: SharedString,
    pub balances_title: SharedString,
    pub balances_match_hero: SharedString,
    pub balances_blind_simulated: SharedString,
    pub balances_best_effort: SharedString,
    pub warn_unlimited: SharedString,
    pub warn_expired: SharedString,
    pub warn_will_fail: SharedString,
    pub warn_hex_message: SharedString,
    pub warn_blind_typed: SharedString,
    pub warn_eth_sign: SharedString,
    pub body_eth_sign: SharedString,
    pub warn_token_to_contract: SharedString,
    pub warn_unverified_amount: SharedString,
    /// What an amount reads when its decimals could not be verified.
    pub amount_unknown: SharedString,
    /// While the core is still resolving. An empty body would read as a
    /// transaction that does nothing.
    pub loading: SharedString,
    /// The pipeline's own three words. Existing keys, all of them: a signing
    /// sheet that says nothing while it works reads as one that hung.
    pub status_signing: SharedString,
    pub status_submitted: SharedString,
    pub error_generic: SharedString,
    pub error_network: SharedString,
    pub error_unlimited: SharedString,
    pub funding_lead: String,
    /// The gas-account top-up, drawn IN the sheet (never a second modal).
    pub funding_title: SharedString,
    pub funding_address_label: SharedString,
    pub funding_amount_label: SharedString,
    pub funding_check_now: SharedString,
    pub funding_confirming: SharedString,
    /// The approval editor's own two sentences.
    pub decimals_unverified: SharedString,
    pub resulting_total_unknown: String,
    pub warn_approve_all: SharedString,
    pub warn_permit_cant_cap: SharedString,
    pub warn_best_effort: SharedString,
    pub warn_verified_abi: SharedString,
    pub warn_sim_unavailable: SharedString,
    /// The two words the simulated balance block needs beyond its title: what
    /// an unverified inflow is called (never its amount — a site can emit any
    /// `Transfer` it likes), and what "it ran and nothing moved" reads as.
    pub balance_unverified_token: SharedString,
    pub sim_no_change: SharedString,
    pub warn_drain: SharedString,
    pub ok_self_transfer: SharedString,
    pub ok_no_network_fee: SharedString,
    pub summary_verified_abi: SharedString,
    pub summary_drain: SharedString,
    pub summary_deploy: SharedString,
    pub summary_safe: SharedString,
    pub fee_label: SharedString,
    pub fee_token_title: SharedString,
    pub fee_balance: SharedString,
    pub tech_function: SharedString,
    pub tech_raw_data: SharedString,
    pub tech_sim_result: SharedString,
    pub tech_identity_token: SharedString,
    pub tech_identity_recipient: SharedString,
    pub copy_value: SharedString,
    pub view_on_explorer: SharedString,
    /// Templates, filled by the fixture layer.
    pub summary_send: String,
    pub summary_send_from: String,
    pub summary_swap: String,
    pub summary_receive: String,
    pub summary_approve: String,
    pub summary_approve_unlimited: String,
    pub summary_revoke: String,
    pub summary_transfer_nft: String,
    pub summary_approve_nft: String,
    pub summary_permit: String,
    pub summary_permit_unlimited: String,
    pub summary_batch: String,
    pub summary_best_effort: String,
    pub warn_blind_decode: String,
    pub warn_selector_not_listed: String,
    pub warn_siwe_mismatch: String,
    pub ok_siwe: String,
    pub self_name: String,
    pub byte_size: String,
    pub safe_inner_call: String,
    pub batch_step: String,
    pub expired_value: String,
    pub tech_param: String,
    pub tech_raw_units: String,
    pub sent_to_token_contract: SharedString,
}

impl SigningStrings {
    #[allow(clippy::too_many_lines, reason = "one line per corpus key")]
    pub fn resolve(loc: &Loc) -> Self {
        let s = |key: &str| loc.t(&format!("componentsUi.signing.{key}"));
        let a = |key: &str| loc.t(&format!("componentsUi.signingApprove.{key}"));
        let raw = |key: &str| loc.t(&format!("componentsUi.signing.{key}")).to_string();
        let raw_a = |key: &str| {
            loc.t(&format!("componentsUi.signingApprove.{key}"))
                .to_string()
        };
        Self {
            panel_title: s("signatureRequest"),
            signing_account: s("signingAccount"),
            advanced_toggle: s("advancedToggle"),
            slide_to_confirm: s("slideToConfirm"),
            confirm_send: s("confirmSend"),
            confirm_swap: s("confirmSwap"),
            confirm_deposit: s("confirmDeposit"),
            confirm_withdraw: s("confirmWithdraw"),
            confirm_plain: s("confirmLabel"),
            sign_label: s("signLabel"),
            intent_send: s("intentSend"),
            intent_approve: s("intentApprove"),
            intent_approve_all: a("verbApproveAll"),
            intent_revoke: s("intentRevoke"),
            intent_swap: s("intentSwap"),
            intent_deposit: s("intentDeposit"),
            intent_withdraw: s("intentWithdraw"),
            intent_transfer_nft: s("intentTransferNft"),
            intent_contract_call: s("intentContractCall"),
            intent_batch: s("batchIntent"),
            intent_blind: s("ethSignIntent"),
            intent_sign_in: s("signInIntent"),
            intent_message: s("messageIntent"),
            intent_typed_data: s("typedDataIntent"),
            intent_permit: s("permitIntent"),
            intent_deploy: s("deployIntent"),
            intent_safe: s("safeIntent"),
            label_recipient: s("recipientLabel"),
            label_spender: s("spenderLabel"),
            label_operator: a("operatorLabel"),
            label_collection: a("collectionLabel"),
            label_interacting: s("interactingLabel"),
            label_from: s("labelFrom"),
            label_amount: s("labelAmount"),
            label_deadline: s("labelDeadline"),
            label_min_received: s("labelMinReceived"),
            label_pay: s("labelPay"),
            label_siwe_site: s("siweDomain"),
            label_siwe_origin: s("siweOrigin"),
            label_siwe_statement: s("siweStatement"),
            label_typed_domain: s("typedDomain"),
            label_type: s("typeLabel"),
            label_signing_for: s("signingFor"),
            label_spending_cap: a("spendingCap"),
            label_expires: a("expiresLabel"),
            label_resulting_total: a("resultingTotal"),
            label_bytecode: s("deployBytecode"),
            label_predicted_address: s("deployPredictedAddress"),
            label_deposit_asset: s("depositAsset"),
            label_shares_received: s("sharesReceived"),
            tag_contact: s("contactTag"),
            tag_wallet: s("walletTag"),
            tag_contract: s("contractTag"),
            tag_verified: s("verifiedTag"),
            tag_unverified: s("unverifiedTag"),
            tag_first_time: s("firstTimeTag"),
            chip_requested: a("requested"),
            chip_balance: a("balanceCap"),
            chip_custom: a("custom"),
            chip_revoke: a("revoke"),
            chip_revoke_access: a("revokeAccess"),
            chip_grant_all: a("grantAllAnyway"),
            value_revoke: a("revokeValue"),
            value_unlimited: a("unlimitedValue"),
            value_all_nfts: a("allNfts"),
            unlimited_disabled: a("unlimitedDisabled"),
            invalid_amount: a("invalidAmount"),
            choose_prompt: a("choosePrompt"),
            balances_title: s("balanceChangesTitle"),
            balances_match_hero: s("balanceMatchesHero"),
            balances_blind_simulated: s("blindButSimulated"),
            balances_best_effort: s("bestEffortSimulated"),
            warn_unlimited: s("unlimitedWarning"),
            warn_expired: s("expiredWarning"),
            warn_will_fail: s("simWillFail"),
            warn_hex_message: s("hexMessageWarning"),
            warn_blind_typed: s("blindTypedWarning"),
            warn_eth_sign: s("ethSignWarning"),
            body_eth_sign: s("ethSignBody"),
            warn_token_to_contract: s("tokenToContractWarning"),
            warn_unverified_amount: s("unverifiedWarning"),
            amount_unknown: s("amountUnknown"),
            loading: s("loading"),
            status_signing: s("signing"),
            status_submitted: s("submitted"),
            // The send flow's own sentence for a submit that failed. One
            // wallet, one way of saying "it did not go out, your funds are
            // safe" — and no raw relay text on a screen (SC-305).
            error_generic: loc.t("send.txErrorGeneric"),
            error_network: loc.t("send.lock.netNotFound"),
            error_unlimited: a("unlimitedDisabled"),
            funding_lead: loc.t("componentsUi.funding.lead").to_string(),
            funding_title: loc.t("componentsUi.funding.title"),
            funding_address_label: loc.t("componentsUi.funding.addressLabel"),
            funding_amount_label: loc.t("componentsUi.funding.amountLabel"),
            funding_check_now: loc.t("componentsUi.funding.checkNow"),
            funding_confirming: loc.t("componentsUi.funding.statusConfirming"),
            decimals_unverified: a("decimalsUnverified"),
            resulting_total_unknown: loc
                .t("componentsUi.signingApprove.resultingTotalUnknown")
                .to_string(),
            warn_approve_all: a("setApprovalAllWarn"),
            warn_permit_cant_cap: a("permitCantCap"),
            warn_best_effort: s("bestEffortWarning"),
            warn_verified_abi: s("verifiedAbiWarning"),
            warn_sim_unavailable: s("simUnavailableWarning"),
            balance_unverified_token: s("balanceUnverifiedToken"),
            sim_no_change: s("simResultNoChange"),
            warn_drain: s("drainWarning"),
            ok_self_transfer: s("balanceSelfTransfer"),
            ok_no_network_fee: s("noNetworkFee"),
            summary_verified_abi: s("verifiedAbiSummary"),
            summary_drain: s("drainSummary"),
            summary_deploy: s("summaryDeploy"),
            summary_safe: s("safeSummary"),
            fee_label: loc.t("componentsUi.gas.networkFee"),
            fee_token_title: s("feeTokenTitle"),
            fee_balance: loc.t("componentsUi.gas.rowBalance"),
            tech_function: s("techFunction"),
            tech_raw_data: s("techRawData"),
            tech_sim_result: s("simResultLabel"),
            tech_identity_token: s("techIdentityToken"),
            tech_identity_recipient: s("techIdentityRecipient"),
            copy_value: s("copyValue"),
            view_on_explorer: s("viewOnExplorer"),
            summary_send: raw("summarySend"),
            summary_send_from: raw("summarySendFrom"),
            summary_swap: raw("summarySwap"),
            summary_receive: raw("summaryReceive"),
            summary_approve: raw_a("capSummary"),
            summary_approve_unlimited: raw("summaryPermitUnlimited"),
            summary_revoke: raw_a("revokeSummary"),
            summary_transfer_nft: raw("summaryTransferNft"),
            summary_approve_nft: raw("summaryApproveNft"),
            summary_permit: raw("summaryPermit"),
            summary_permit_unlimited: raw("summaryPermitUnlimited"),
            summary_batch: raw("batchSubtitle"),
            summary_best_effort: raw("bestEffortSummary"),
            warn_blind_decode: raw("blindDecodeWarning"),
            warn_selector_not_listed: raw("selectorNotListed"),
            warn_siwe_mismatch: raw("siweMismatch"),
            ok_siwe: raw("siweOk"),
            self_name: raw("selfName"),
            byte_size: raw("byteSize"),
            safe_inner_call: raw("safeInnerCall"),
            batch_step: raw("batchStep"),
            expired_value: raw("expiredValue"),
            tech_param: raw("techParam"),
            tech_raw_units: raw("techRawUnits"),
            sent_to_token_contract: s("sendingToTokenContract"),
        }
    }
}

/// `{{var}}` interpolation for the signing templates — the same one-line fill
/// the wallet strings use, not a parallel i18n engine.
pub fn fill(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = template.to_owned();
    for (name, value) in vars {
        out = out.replace(&format!("{{{{{name}}}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_strings_resolve_without_echo() {
        let loc = Loc::from_env();
        let s = SigningStrings::resolve(&loc);
        for (value, key) in [
            (
                s.panel_title.as_ref(),
                "componentsUi.signing.signatureRequest",
            ),
            (
                s.slide_to_confirm.as_ref(),
                "componentsUi.signing.slideToConfirm",
            ),
            (s.warn_drain.as_ref(), "componentsUi.signing.drainWarning"),
            (
                s.value_unlimited.as_ref(),
                "componentsUi.signingApprove.unlimitedValue",
            ),
        ] {
            assert_ne!(value, key, "`{key}` echoed the key");
        }
        assert!(s.summary_send.contains("{{amount}}"));
        assert!(s.byte_size.contains("{{n}}"));
    }

    #[test]
    fn fill_replaces_named_vars() {
        assert_eq!(fill("{{a}} → {{b}}", &[("a", "x"), ("b", "y")]), "x → y");
    }
}
