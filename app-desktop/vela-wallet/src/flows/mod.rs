//! Wallet flows (spec 021): Receive, Send, Activity and Assets, as third-column
//! panels over the wallet home.
//!
//! Same split as `wallet/`: `fixtures` is the canonical data (verbatim mock
//! content, the same canon the other three clients port), `components` the
//! reusable visuals (theme + resolved strings in, `Div` out), `panels` the
//! bodies the page drops into its existing `panel_scaffold`.
//!
//! The panels are a STACK, not a single id. The mocks stack: Receive opens a
//! network list and a network opens its QR; Send runs picker → form → confirm →
//! receipt. DR2L, DA2L, DT3L and DSD2L all draw a back chevron beside the panel
//! title, and a chevron has to lead somewhere.

pub mod components;
pub mod eip681;
pub mod fixtures;
pub mod live;
pub mod panels;
pub mod share_card;

use gpui::SharedString;

use crate::loc::Loc;

/// Which flow panel the third column holds.
///
/// The ids are the desktop half of spec.md's state matrix. `Ds1` is the one
/// that is NOT a panel — a scanner is a viewfinder and a 400px column is the
/// wrong shape for one, so the page draws it as a centred modal (DS1L).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FlowPanel {
    Dr1,
    Dr2,
    Dr3,
    Ds1,
    Da1,
    Da2,
    Da3,
    Dt1,
    Dt3,
    Dt3b,
    Dt4,
    Dsd1,
    Dsd2,
    Dsd2b,
    Dsd2c,
    Dsd2e,
    Dsd2f,
    Dsd3,
    Dsd4,
}

impl FlowPanel {
    /// The chip strip and the inventory test read one array, so the gallery and
    /// the test can never disagree about which states exist.
    pub const ALL: [(FlowPanel, &'static str); 19] = [
        (FlowPanel::Dr1, "DR1"),
        (FlowPanel::Dr2, "DR2"),
        (FlowPanel::Dr3, "DR3"),
        (FlowPanel::Ds1, "DS1"),
        (FlowPanel::Da1, "DA1"),
        (FlowPanel::Da2, "DA2"),
        (FlowPanel::Da3, "DA3"),
        (FlowPanel::Dt1, "DT1"),
        (FlowPanel::Dt3, "DT3"),
        (FlowPanel::Dt3b, "DT3b"),
        (FlowPanel::Dt4, "DT4"),
        (FlowPanel::Dsd1, "DSD1"),
        (FlowPanel::Dsd2, "DSD2"),
        (FlowPanel::Dsd2b, "DSD2b"),
        (FlowPanel::Dsd2c, "DSD2c"),
        (FlowPanel::Dsd2e, "DSD2e"),
        (FlowPanel::Dsd2f, "DSD2f"),
        (FlowPanel::Dsd3, "DSD3"),
        (FlowPanel::Dsd4, "DSD4"),
    ];

    /// `VELA_FLOW=DSD2` opens the window straight onto one panel.
    ///
    /// Same dev-seam family as `VELA_GALLERY_STATE`: reviewing a panel should
    /// not require clicking three levels down to it, and a headless shell can
    /// screenshot a state it cannot click to.
    pub fn from_env() -> Option<FlowPanel> {
        let want = std::env::var("VELA_FLOW").ok()?;
        FlowPanel::ALL
            .iter()
            .find(|(_, label)| label.eq_ignore_ascii_case(want.trim()))
            .map(|(panel, _)| *panel)
    }

    /// The stack that leaves this panel on top with a truthful chevron under
    /// it — the path a person would have walked to reach it.
    pub fn stack(self) -> Vec<FlowPanel> {
        match self {
            FlowPanel::Dr2 | FlowPanel::Dr3 => vec![FlowPanel::Dr1, self],
            FlowPanel::Da2 | FlowPanel::Da3 => vec![FlowPanel::Da1, self],
            FlowPanel::Dt3 | FlowPanel::Dt3b => vec![FlowPanel::Dt1, self],
            FlowPanel::Dsd2 | FlowPanel::Dsd2b => vec![FlowPanel::Dsd1, self],
            FlowPanel::Dsd2c | FlowPanel::Dsd2e | FlowPanel::Dsd2f => {
                vec![FlowPanel::Dsd1, FlowPanel::Dsd2, self]
            }
            FlowPanel::Dsd3 => vec![FlowPanel::Dsd1, FlowPanel::Dsd2, self],
            FlowPanel::Dsd4 => vec![FlowPanel::Dsd1, FlowPanel::Dsd2, FlowPanel::Dsd3, self],
            _ => vec![self],
        }
    }

    /// The stack an entry from the wallet home opens, deepest last.
    ///
    /// `AddToken` opens two: the assets panel and the add sheet over it. That
    /// is what makes DT3L's back chevron lead to the list you were adding to
    /// rather than out of the column entirely.
    pub fn entry(entry: FlowEntry) -> Vec<FlowPanel> {
        match entry {
            FlowEntry::Receive => vec![FlowPanel::Dr1],
            FlowEntry::Send => vec![FlowPanel::Dsd1],
            FlowEntry::Scan => vec![FlowPanel::Ds1],
            FlowEntry::Activity => vec![FlowPanel::Da1],
            FlowEntry::Assets => vec![FlowPanel::Dt1],
            FlowEntry::AddToken => vec![FlowPanel::Dt1, FlowPanel::Dt3],
            FlowEntry::TxDetail => vec![FlowPanel::Da1, FlowPanel::Da2],
        }
    }

    /// One step deeper from the panel currently on top.
    pub fn step(self, step: FlowStep) -> Option<FlowPanel> {
        match (self, step) {
            (FlowPanel::Dr1, FlowStep::ReceiveQr) => Some(FlowPanel::Dr2),
            (FlowPanel::Da1, FlowStep::TxDetail) => Some(FlowPanel::Da2),
            (FlowPanel::Dt1, FlowStep::AddToken) => Some(FlowPanel::Dt3),
            (FlowPanel::Dsd1, FlowStep::SendForm) => Some(FlowPanel::Dsd2),
            (FlowPanel::Dsd2 | FlowPanel::Dsd2b, FlowStep::SendConfirm) => Some(FlowPanel::Dsd3),
            (FlowPanel::Dsd3, FlowStep::SendReceipt) => Some(FlowPanel::Dsd4),
            (FlowPanel::Dsd2 | FlowPanel::Dsd2b, FlowStep::ContactPick) => Some(FlowPanel::Dsd2e),
            (FlowPanel::Dsd2 | FlowPanel::Dsd2b, FlowStep::FeeToken) => Some(FlowPanel::Dsd2f),
            (FlowPanel::Dsd2b, FlowStep::BatchImport) => Some(FlowPanel::Dsd2c),
            (FlowPanel::Dsd2, FlowStep::AddRecipient) => Some(FlowPanel::Dsd2b),
            (_, FlowStep::Scan) => Some(FlowPanel::Ds1),
            _ => None,
        }
    }
}

/// Where a flow can be entered from the wallet home.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlowEntry {
    Receive,
    Send,
    Scan,
    Activity,
    Assets,
    AddToken,
    TxDetail,
}

/// Where a panel can go next. Names match the other three clients' intents.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlowStep {
    ReceiveQr,
    TxDetail,
    AddToken,
    SendForm,
    SendConfirm,
    SendReceipt,
    ContactPick,
    FeeToken,
    BatchImport,
    AddRecipient,
    Scan,
}

/// Every wallet-flow string, resolved once per locale.
///
/// Most of this vocabulary already existed — the legacy React Native app left
/// `receive.*`, `send.*`, `history.*`, `assets.*`, `addToken.*` and
/// `componentsTx.*` in the corpus. Only the thirty-three the mocks genuinely
/// added are new.
///
/// Every string the feature resolves, whether or not this client draws it. The
/// desktop column hosts nineteen of the thirty states; the share card, the
/// mobile network-filter pill, the identicon viewer, the sweep selector and the
/// searched-to-empty variants are in the mobile matrix, and their strings
/// resolve here so the resolver stays one list against the spec's key table.
/// Deleting the unread ones would make the next desktop panel re-derive them.
#[allow(dead_code)]
pub struct FlowStrings {
    // Chrome.
    pub back: SharedString,
    pub close: SharedString,
    pub copy_address: SharedString,
    pub pill_all: SharedString,
    pub today: SharedString,
    pub yesterday: SharedString,

    // Receive.
    pub receive_title: SharedString,
    pub receive_search: SharedString,
    /// Templates carrying `{{count}}` / `{{query}}` / `{{network}}`.
    pub networks_line: String,
    pub search_empty: String,
    pub qr_title_network: String,
    pub qr_title_asset: String,
    pub share_card_note: String,
    pub token_contract: SharedString,
    pub warning_reminder: SharedString,
    /// The gate a person passes once per account, in the corpus's words.
    pub warning_title: SharedString,
    pub warning_body: SharedString,
    pub warning_counterfactual: SharedString,
    pub warning_confirm: SharedString,
    pub save_image: SharedString,
    pub share_card_headline: SharedString,

    // Scan.
    pub scan_title: SharedString,
    pub scan_hint: SharedString,
    pub scan_from_gallery: SharedString,
    pub scan_flip: SharedString,

    // Activity.
    pub history_title: SharedString,
    pub history_empty_filter: SharedString,
    pub label_sent: SharedString,
    pub label_received: SharedString,
    pub tx_label_sent: String,
    pub tx_label_received: String,
    pub to_name: String,
    pub from_name: String,
    pub view_on_explorer: SharedString,
    pub status_confirmed: SharedString,
    /// A pending or failed transfer must not wear the confirmed chip. Same key
    /// family the RN `TxStatusBadge` reads, so the three clients say the same
    /// word about the same state.
    pub status_pending: SharedString,
    pub status_failed: SharedString,
    pub detail_from: SharedString,
    pub detail_to: SharedString,
    pub detail_chain: SharedString,
    pub detail_date: SharedString,
    pub detail_hash: SharedString,
    pub detail_section_title: SharedString,

    // Assets.
    pub assets_title: SharedString,
    pub assets_add: SharedString,
    pub assets_search: SharedString,
    pub add_by_address: SharedString,
    pub assets_empty_title: SharedString,
    pub assets_empty_caption: SharedString,
    pub not_showing_title: SharedString,
    pub not_showing_body: SharedString,

    // Add token.
    pub add_token_title: SharedString,
    pub tab_erc20: SharedString,
    pub tab_native: SharedString,
    pub label_network: SharedString,
    pub label_decimals: SharedString,
    pub token_address_label: SharedString,
    pub add_to_wallet: SharedString,
    /// DT3L live states. "Not found" and "not searched yet" are different
    /// answers, and a card that says neither is a card that says nothing.
    pub not_found_title: SharedString,
    pub not_found_message: SharedString,
    pub searching_networks: SharedString,
    pub search_token_btn: SharedString,
    pub net_search_label: SharedString,
    pub net_search_placeholder: SharedString,
    pub net_picker_search: SharedString,
    pub label_chain_id: SharedString,
    pub label_native_token: SharedString,
    pub compatible: SharedString,
    pub add_network_btn: SharedString,
    pub add_token_error_title: SharedString,
    pub add_token_error_save: SharedString,

    // Send.
    /// The plain verb, not the "Send {{symbol}}" template — DSD4L's bar keeps
    /// the journey's name while the body carries the state.
    pub send_action: SharedString,
    pub select_token_title: SharedString,
    pub send_search: SharedString,
    pub filter_all: SharedString,
    pub filter_stable: SharedString,
    pub filter_gas: SharedString,
    pub filter_other: SharedString,
    pub multi_send_title: SharedString,
    /// The sweep picker's three sentences. `chain_notice` and `continue` are
    /// templates (`{{network}}`, `{{n}}` + `{{chain}}`) — the phone drew this
    /// screen, so every word is already translated.
    pub multi_send_chain_notice: String,
    pub multi_send_continue: String,
    pub select_all_valuable: SharedString,
    /// Templates carrying `{{symbol}}` / `{{amount}}` / `{{n}}` / `{{count}}`.
    pub send_title: String,
    pub balance_label: String,
    pub recipient_n: String,
    pub recipient_count: String,
    pub max: SharedString,
    pub recipient_label: SharedString,
    pub add_recipient: SharedString,
    pub from_contacts: SharedString,
    pub batch_import: SharedString,
    pub remove_recipient: SharedString,
    pub recipient_pick: SharedString,
    pub split_total: SharedString,
    pub continue_btn: SharedString,
    pub network_fee: SharedString,

    // Send · fee token.
    pub fee_token_label: SharedString,
    pub fee_token_hint: SharedString,
    pub fee_token_estimate: SharedString,

    // Send · contact picker.
    pub pick_contact_title: SharedString,
    pub pick_contact_search: SharedString,
    pub scan_to_fill: SharedString,
    pub contacts_groups: SharedString,
    pub contacts_title: SharedString,
    pub group_members: String,

    // Send · batch import.
    pub batch_title: SharedString,
    pub batch_unit_fiat: String,
    pub batch_unit_token: String,
    pub batch_import_file: SharedString,
    pub batch_template: SharedString,
    pub batch_rate_section: SharedString,
    pub batch_rate_label: String,
    pub batch_rate_hint: String,
    pub batch_parsed: String,
    pub batch_bad_address: SharedString,
    pub batch_rejected_one: String,
    pub batch_apply: String,

    // Send · confirm.
    pub confirm_title: SharedString,
    pub from_label: SharedString,
    pub to_label: SharedString,
    pub est_fee: SharedString,
    pub confirm_send: SharedString,
    pub confirm_total_line: String,
    pub assets_count: String,

    // Send · receipt.
    pub tx_submitted_title: SharedString,
    pub tx_waiting_confirm: SharedString,
    /// The two holds the core distinguishes on a receipt
    /// (`SendReceiptView.hold_reason`). Both sentences were already in the
    /// corpus and no shell — desktop or web — was saying either.
    pub tx_held_fees: SharedString,
    pub tx_rejected_fees: SharedString,
    pub tx_typical_time: String,
    /// Spec 038 #D3: the two lines beside the usual-time sentence.
    pub tx_slow_confirm: SharedString,
    pub tx_elapsed: String,
    pub recipient_count_other: String,
    pub tx_close_background: SharedString,
    pub tx_hash: SharedString,
    pub done: SharedString,

    // Send · live (spec 032). The receipt's other three states, the two error
    // wordings the core chooses between, and the recipient trust line.
    /// Template carrying `{{amount}}` / `{{symbol}}`.
    pub tx_confirmed_title: String,
    pub tx_submitting: SharedString,
    pub tx_preparing: SharedString,
    pub tx_background_hint: SharedString,
    pub tx_error_generic: SharedString,
    pub tx_error_bundler_fund: SharedString,
    pub first_time_tag: SharedString,
    pub fee_pending: SharedString,

    // Send · the core's refusals, live (spec 032 phase 6). Every one of these
    // is a sentence the core computed and this client used to throw away: a
    // person over-typing their balance saw a button that would not move and
    // nothing that said why.
    /// Templates carrying `{{symbol}}` / `{{sym}}` / `{{code}}`.
    pub warn_not_enough_token: String,
    pub warn_insufficient_for_gas: String,
    pub warn_need_gas: String,
    pub warn_cannot_convert: String,
    /// The same-asset fee ceiling: the transfer and its fee draw on one coin.
    pub same_fee_title: String,
    pub same_fee_body: String,
    pub same_fee_max: String,
    pub same_fee_edit: SharedString,
    pub insufficient_title: SharedString,
    pub insufficient_body: SharedString,
    pub estimating: SharedString,
    /// The relay's float is empty and someone has to top it up before this
    /// chain can carry anything.
    pub funding_title: SharedString,
    pub funding_lead: String,
    pub funding_address_label: SharedString,
    pub funding_amount_label: SharedString,
    pub funding_check_now: SharedString,
    /// "Not now" — the phone's own word for leaving this stop. The desktop
    /// draws the stop inline rather than as a sheet, so this is the way back
    /// to the form.
    pub funding_close: SharedString,
    /// A locked payment request that cannot be fulfilled.
    pub lock_net_title: SharedString,
    pub lock_net_body: String,
    pub lock_token_title: SharedString,
    pub lock_token_body: SharedString,
    pub lock_add_network: SharedString,
    pub lock_net_not_found: SharedString,
    pub lock_net_not_compatible: SharedString,
    pub lock_net_add_error: SharedString,

    // Send · batch import, live (spec 032 phase 5).
    pub batch_paste_placeholder: SharedString,
    pub batch_template_saved: SharedString,
    pub batch_rate_loading: SharedString,
    pub batch_rate_failed: SharedString,
    pub batch_rate_reset: SharedString,
    pub batch_over_cap: SharedString,
    pub batch_over_balance: SharedString,
    pub batch_reading: SharedString,
    pub batch_rejected_other: String,
    pub batch_apply_empty: SharedString,
    pub batch_no_price: SharedString,
    pub batch_import_failed_title: SharedString,
    pub batch_import_failed_body: SharedString,
}

impl FlowStrings {
    pub fn resolve(loc: &Loc) -> Self {
        let s = |key: &str| loc.t(key);
        let raw = |key: &str| loc.t(key).to_string();
        Self {
            back: s("receive.a11yBack"),
            close: s("componentsUi.identiconViewer.close"),
            copy_address: s("componentsUi.identiconViewer.copyAddress"),
            pill_all: s("componentsUi.networkFilter.pillAll"),
            today: s("componentsUi.dayGroup.today"),
            yesterday: s("componentsUi.dayGroup.yesterday"),

            receive_title: s("receive.title"),
            receive_search: s("receive.searchNetworkPlaceholder"),
            networks_line: raw("receive.networksLine"),
            search_empty: raw("receive.searchNetworkEmpty"),
            qr_title_network: raw("receive.qrTitleNetwork"),
            qr_title_asset: raw("receive.qrTitleAsset"),
            share_card_note: raw("receive.shareCardNetworkNote"),
            token_contract: s("receive.tokenContract"),
            warning_reminder: s("receive.warningReminder"),
            warning_title: s("receive.warningTitle"),
            warning_body: s("receive.warningBody"),
            warning_counterfactual: s("receive.warningCounterfactual"),
            warning_confirm: s("receive.warningConfirm"),
            save_image: s("receive.request.saveImage"),
            share_card_headline: s("receive.shareCardHeadline"),

            scan_title: s("componentsUi.scanner.title"),
            scan_hint: s("componentsUi.scanner.hint"),
            scan_from_gallery: s("componentsUi.scanner.fromGallery"),
            scan_flip: s("componentsUi.scanner.flipCamera"),

            history_title: s("history.navTitle"),
            history_empty_filter: s("history.emptyFilter"),
            label_sent: s("history.labelSent"),
            label_received: s("history.labelReceived"),
            tx_label_sent: raw("history.txLabelSent"),
            tx_label_received: raw("history.txLabelReceived"),
            to_name: raw("history.toName"),
            from_name: raw("history.fromName"),
            view_on_explorer: s("history.viewOnExplorer"),
            status_confirmed: s("componentsTx.receipt.statusConfirmed"),
            status_pending: s("componentsTx.detail.statusPending"),
            status_failed: s("componentsTx.detail.statusFailed"),
            detail_from: s("componentsTx.detail.from"),
            detail_to: s("componentsTx.detail.to"),
            detail_chain: s("componentsTx.detail.labelChain"),
            detail_date: s("componentsTx.detail.labelDate"),
            detail_hash: s("componentsTx.detail.labelHash"),
            detail_section_title: s("componentsTx.detail.sectionTitle"),

            assets_title: s("assets.sectionTitle"),
            assets_add: s("assets.addToken"),
            assets_search: s("assets.searchPlaceholder"),
            add_by_address: s("assets.addByAddress"),
            assets_empty_title: s("assets.emptyTitle"),
            assets_empty_caption: s("assets.emptySubtext"),
            not_showing_title: s("assets.notShowingTitle"),
            not_showing_body: s("assets.notShowingBody"),

            add_token_title: s("addToken.navTitle"),
            tab_erc20: s("addToken.tabErc20"),
            tab_native: s("addToken.tabNative"),
            label_network: s("addToken.labelNetwork"),
            label_decimals: s("addToken.labelDecimals"),
            token_address_label: s("addToken.tokenAddressLabel"),
            add_to_wallet: s("addToken.addToWalletBtn"),
            not_found_title: s("addToken.notFoundTitle"),
            not_found_message: s("addToken.notFoundMessage"),
            searching_networks: s("addToken.searchingNetworks"),
            search_token_btn: s("addToken.searchTokenBtn"),
            net_search_label: s("addToken.netSearchLabel"),
            net_search_placeholder: s("addToken.netSearchPlaceholder"),
            net_picker_search: s("addToken.netPickerSearchPlaceholder"),
            label_chain_id: s("addToken.labelChainId"),
            label_native_token: s("addToken.labelNativeToken"),
            compatible: s("addToken.compatible"),
            add_network_btn: s("addToken.addNetworkBtn"),
            add_token_error_title: s("addToken.errorTitle"),
            add_token_error_save: s("addToken.errorSaveToken"),

            send_action: s("componentsUi.dock.send"),
            select_token_title: s("send.selectTokenTitle"),
            send_search: s("send.searchPlaceholder"),
            filter_all: s("history.filterAll"),
            filter_stable: s("send.filterStable"),
            filter_gas: s("send.filterGas"),
            filter_other: s("send.filterOther"),
            multi_send_title: s("send.multiSendTitle"),
            multi_send_chain_notice: raw("send.multiSendChainNotice"),
            multi_send_continue: raw("send.multiSendContinue"),
            select_all_valuable: s("send.selectAllValuable"),
            send_title: raw("send.sendTitle"),
            balance_label: raw("send.balanceLabel"),
            recipient_n: raw("send.recipientN"),
            recipient_count: raw("send.recipientCount_other"),
            max: s("send.maxBtn"),
            recipient_label: s("send.recipientLabel"),
            add_recipient: s("send.addRecipient"),
            from_contacts: s("send.fromContacts"),
            batch_import: s("send.batchImport"),
            remove_recipient: s("send.removeRecipient"),
            recipient_pick: s("send.recipientPickAria"),
            split_total: s("send.splitTotalLabel"),
            continue_btn: s("send.continueBtn"),
            network_fee: s("componentsUi.gas.networkFee"),

            fee_token_label: s("send.feeTokenLabel"),
            fee_token_hint: s("send.feeTokenHint"),
            fee_token_estimate: s("send.feeTokenEstimate"),

            pick_contact_title: s("send.pickContactTitle"),
            pick_contact_search: s("send.pickContactSearch"),
            scan_to_fill: s("send.scanToFill"),
            contacts_groups: s("contacts.sectionGroups"),
            contacts_title: s("contacts.title"),
            group_members: raw("contacts.groupMembers"),

            batch_title: s("send.batchTitle"),
            batch_unit_fiat: raw("send.batchUnitFiat"),
            batch_unit_token: raw("send.batchUnitToken"),
            batch_import_file: s("send.batchImportFile"),
            batch_template: s("send.batchTemplate"),
            batch_rate_section: s("send.batchRateSection"),
            batch_rate_label: raw("send.batchRateLabel"),
            batch_rate_hint: raw("send.batchRateHint"),
            batch_parsed: raw("send.batchParsedCount"),
            batch_bad_address: s("send.batchBadAddress"),
            batch_rejected_one: raw("send.batchRejected_one"),
            batch_apply: raw("send.batchApply_other"),

            confirm_title: s("send.confirmTitle"),
            from_label: s("send.fromLabel"),
            to_label: s("send.toLabel"),
            est_fee: s("send.estFeeLabel"),
            confirm_send: s("send.confirmSendBtn"),
            confirm_total_line: raw("send.confirmTotalLine"),
            assets_count: raw("componentsTx.receipt.assetsCount"),

            tx_submitted_title: s("send.txSubmittedTitle"),
            tx_waiting_confirm: s("send.txWaitingConfirm"),
            tx_held_fees: s("send.txHeldFees"),
            tx_rejected_fees: s("send.txRejectedFees"),
            tx_typical_time: raw("send.txTypicalTime"),
            tx_slow_confirm: s("send.txSlowConfirm"),
            tx_elapsed: raw("send.txElapsed"),
            recipient_count_other: raw("send.recipientCount_other"),
            tx_close_background: s("send.txCloseBackground"),
            tx_hash: s("componentsTx.receipt.txHash"),
            done: s("componentsTx.receipt.done"),

            tx_confirmed_title: raw("send.txConfirmedTitle"),
            tx_submitting: s("send.txSubmitting"),
            tx_preparing: s("send.txPreparingBiometric"),
            tx_background_hint: s("send.txBackgroundHint"),
            tx_error_generic: s("send.txErrorGeneric"),
            tx_error_bundler_fund: s("send.txErrorBundlerFund"),
            first_time_tag: s("componentsUi.signing.firstTimeTag"),
            fee_pending: SharedString::from("…"),

            warn_not_enough_token: raw("send.warnNotEnoughToken"),
            warn_insufficient_for_gas: raw("send.warnInsufficientForGas"),
            warn_need_gas: raw("send.warnNeedGas"),
            warn_cannot_convert: raw("send.warnCannotConvert"),
            same_fee_title: raw("send.sameFeeTokenTitle"),
            same_fee_body: raw("send.sameFeeTokenBody"),
            same_fee_max: raw("send.sameFeeTokenMax"),
            same_fee_edit: s("send.sameFeeTokenEdit"),
            insufficient_title: s("send.alertInsufficientBalanceTitle"),
            insufficient_body: s("send.alertInsufficientBalanceBody"),
            estimating: s("componentsUi.gas.estimating"),
            funding_title: s("componentsUi.funding.title"),
            funding_lead: raw("componentsUi.funding.lead"),
            funding_address_label: s("componentsUi.funding.addressLabel"),
            funding_amount_label: s("componentsUi.funding.amountLabel"),
            funding_check_now: s("componentsUi.funding.checkNow"),
            funding_close: s("componentsUi.funding.cancel"),
            lock_net_title: s("send.lock.netTitle"),
            lock_net_body: raw("send.lock.netBody"),
            lock_token_title: s("send.lock.tokenTitle"),
            lock_token_body: s("send.lock.tokenBody"),
            lock_add_network: s("send.lock.addNetwork"),
            lock_net_not_found: s("send.lock.netNotFound"),
            lock_net_not_compatible: s("send.lock.netNotCompatible"),
            lock_net_add_error: s("send.lock.netAddError"),

            batch_paste_placeholder: s("send.batchPastePlaceholder"),
            batch_template_saved: s("send.batchTemplateSaved"),
            batch_rate_loading: s("send.batchRateLoading"),
            batch_rate_failed: s("send.batchRateFailed"),
            batch_rate_reset: s("send.batchRateReset"),
            batch_over_cap: s("send.batchOverCap"),
            batch_over_balance: s("send.batchOverBalance"),
            batch_reading: s("send.batchReading"),
            batch_rejected_other: raw("send.batchRejected_other"),
            batch_apply_empty: s("send.batchApplyEmpty"),
            batch_no_price: s("send.batchNoPrice"),
            batch_import_failed_title: s("send.batchImportFailedTitle"),
            batch_import_failed_body: s("send.batchImportFailedBody"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SC-004 discipline: none of the flow keys may echo.
    #[test]
    fn flow_strings_resolve_without_echo() {
        let loc = Loc::from_env();
        let s = FlowStrings::resolve(&loc);
        for (value, key) in [
            (s.receive_title.as_ref(), "receive.title"),
            (
                s.receive_search.as_ref(),
                "receive.searchNetworkPlaceholder",
            ),
            (s.token_contract.as_ref(), "receive.tokenContract"),
            (s.add_by_address.as_ref(), "assets.addByAddress"),
            (s.not_showing_title.as_ref(), "assets.notShowingTitle"),
            (s.from_contacts.as_ref(), "send.fromContacts"),
            (s.fee_token_hint.as_ref(), "send.feeTokenHint"),
            (s.pick_contact_title.as_ref(), "send.pickContactTitle"),
            (s.tx_submitted_title.as_ref(), "send.txSubmittedTitle"),
            (
                s.scan_from_gallery.as_ref(),
                "componentsUi.scanner.fromGallery",
            ),
        ] {
            assert_ne!(value, key, "`{key}` echoed the key");
        }
        for (template, var) in [
            (&s.networks_line, "{{count}}"),
            (&s.search_empty, "{{query}}"),
            (&s.qr_title_network, "{{network}}"),
            (&s.balance_label, "{{amount}}"),
            (&s.confirm_total_line, "{{fiat}}"),
        ] {
            assert!(template.contains(var), "`{template}` must carry {var}");
        }
    }

    /// The stack is what makes a back chevron mean something.
    #[test]
    fn entries_open_the_right_depth() {
        assert_eq!(FlowPanel::entry(FlowEntry::Receive), vec![FlowPanel::Dr1]);
        // Two deep, so DT3L's chevron leads to the list you were adding to.
        assert_eq!(
            FlowPanel::entry(FlowEntry::AddToken),
            vec![FlowPanel::Dt1, FlowPanel::Dt3]
        );
        assert_eq!(
            FlowPanel::entry(FlowEntry::TxDetail),
            vec![FlowPanel::Da1, FlowPanel::Da2]
        );
    }

    #[test]
    fn steps_only_apply_where_the_mocks_draw_them() {
        assert_eq!(
            FlowPanel::Dr1.step(FlowStep::ReceiveQr),
            Some(FlowPanel::Dr2)
        );
        assert_eq!(
            FlowPanel::Dsd2.step(FlowStep::FeeToken),
            Some(FlowPanel::Dsd2f)
        );
        // A step with nowhere to go does nothing rather than panicking.
        assert_eq!(FlowPanel::Dr1.step(FlowStep::SendConfirm), None);
    }

    /// Every panel is either walked to from an entry, or is a variant of one
    /// that is — a second row, another tab, the same list gone empty.
    ///
    /// This is the guard against the defect the whole wiring pass was about: a
    /// panel that only the gallery can reach is a panel the product cannot,
    /// and the mocks draw an affordance for each of these.
    #[test]
    fn every_panel_is_reachable_or_a_named_variant() {
        use std::collections::HashSet;

        const ENTRIES: [FlowEntry; 7] = [
            FlowEntry::Receive,
            FlowEntry::Send,
            FlowEntry::Scan,
            FlowEntry::Activity,
            FlowEntry::Assets,
            FlowEntry::AddToken,
            FlowEntry::TxDetail,
        ];
        const STEPS: [FlowStep; 11] = [
            FlowStep::ReceiveQr,
            FlowStep::TxDetail,
            FlowStep::AddToken,
            FlowStep::SendForm,
            FlowStep::SendConfirm,
            FlowStep::SendReceipt,
            FlowStep::ContactPick,
            FlowStep::FeeToken,
            FlowStep::BatchImport,
            FlowStep::AddRecipient,
            FlowStep::Scan,
        ];
        /// Same panel, different content — the asset QR (a row in DR1), the
        /// outgoing transaction (a row in DA1), the empty asset list, and the
        /// add-token form's native tab. Each is a state of a reachable panel
        /// rather than a place a step leads to.
        const VARIANTS: [FlowPanel; 4] = [
            FlowPanel::Dr3,
            FlowPanel::Da3,
            FlowPanel::Dt4,
            FlowPanel::Dt3b,
        ];

        let mut seen: HashSet<FlowPanel> = HashSet::new();
        let mut queue: Vec<FlowPanel> = ENTRIES.iter().flat_map(|e| FlowPanel::entry(*e)).collect();
        while let Some(panel) = queue.pop() {
            if !seen.insert(panel) {
                continue;
            }
            for step in STEPS {
                if let Some(next) = panel.step(step) {
                    queue.push(next);
                }
            }
        }
        for (panel, label) in FlowPanel::ALL {
            assert!(
                seen.contains(&panel) || VARIANTS.contains(&panel),
                "{label} is reachable from neither an entry nor a step"
            );
        }
    }

    /// The chip strip and this array are the same list, so a state cannot be
    /// added to one and forgotten in the other.
    #[test]
    fn every_panel_is_in_the_chip_strip() {
        assert_eq!(FlowPanel::ALL.len(), 19);
        let labels: Vec<_> = FlowPanel::ALL.iter().map(|(_, label)| *label).collect();
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(unique.len(), labels.len(), "chip labels must be unique");
    }
}
