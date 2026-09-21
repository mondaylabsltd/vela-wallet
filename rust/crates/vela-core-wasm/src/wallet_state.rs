//! wasm classes for the wallet-state machines (spec 016-crux-wallet-state).
//!
//! Each is the shared [`crate::bridge::Bridge`] over one machine — JSON in,
//! JSON out, no business logic here. See
//! `specs/016-crux-wallet-state/contracts/wallet-state-core.md`.

use crate::bridge::bridge_class;

bridge_class!(
    /// The display currency: atomic code+rate pair, first-launch region seed,
    /// user-choice-wins.
    DisplayCurrencyCore,
    vela_core::app::display_currency::DisplayCurrency
);

bridge_class!(
    /// Deposit detection on the Receive screen: phased polling, baseline
    /// diff, false-positive guards.
    ReceiveWatchCore,
    vela_core::app::receive_watch::ReceiveWatch
);

bridge_class!(
    /// Payment requests: the acknowledge gate, the EIP-681/pay-link builder,
    /// and the strict `/pay` validator.
    PaymentRequestCore,
    vela_core::app::payment_request::PaymentRequest
);

bridge_class!(
    /// Fee quoting + reserve math (spec 017 wave A): tier pricing, in-band
    /// quotes, sign-what-was-displayed guards.
    FeePolicyCore,
    vela_core::app::fee_policy::FeePolicy
);

bridge_class!(
    /// The default transaction speed (spec 068): the stored tier a send
    /// starts at, the factory `fast` when nothing was chosen.
    FeeTierPrefCore,
    vela_core::app::fee_tier_pref::FeeTierPref
);

bridge_class!(
    /// The speed control of one send surface (spec 069): the tier in force,
    /// the free upgrade, the one-speed statement and each tier's gas bid.
    FeeSpeedCore,
    vela_core::app::fee_speed::FeeSpeed
);

bridge_class!(
    /// Never-unlimited approval guard and allowance editor.
    ApprovalGuardCore,
    vela_core::app::approval_guard::ApprovalGuard
);

bridge_class!(
    /// Clear-signing resolution pipeline and message risk verdicts.
    ClearSigningCore,
    vela_core::app::clear_signing::ClearSigning
);

bridge_class!(
    /// Post-submit transaction lifecycle / reconciliation.
    TxTrackerCore,
    vela_core::app::tx_tracker::TxTracker
);

bridge_class!(
    /// The address book: manual + history-derived merge, tombstones, groups.
    ContactsCore,
    vela_core::app::contacts::Contacts
);

bridge_class!(
    /// Payroll batch import: table interpretation, fiat conversion, caps.
    BatchImportCore,
    vela_core::app::batch_import::BatchImport
);

bridge_class!(
    /// The wallet session truth source: accounts, active index, boot restore.
    SessionCore,
    vela_core::app::session::Session
);

bridge_class!(
    /// Balance aggregation & display policy (per active account).
    BalanceDashboardCore,
    vela_core::app::balance_dashboard::BalanceDashboard
);

bridge_class!(
    /// RPC/bundler endpoint pool decisions: scoring, cooldowns, bans.
    RpcPoolCore,
    vela_core::app::rpc_pool::RpcPool
);

bridge_class!(
    /// The token trust model: transfer allowlists, auto-add admission,
    /// asymmetric simulation trust.
    TokenTrustCore,
    vela_core::app::token_trust::TokenTrust
);

bridge_class!(
    /// Network & endpoint configuration: add-network wizard, overrides,
    /// service endpoints, provider keys.
    NetworkAdminCore,
    vela_core::app::network_admin::NetworkAdmin
);

bridge_class!(
    /// The dApp signing approval lifecycle.
    SignRequestCore,
    vela_core::app::sign_request::SignRequest
);

bridge_class!(
    /// Per-origin grants + browser consent.
    DappPermissionsCore,
    vela_core::app::dapp_permissions::DappPermissions
);

bridge_class!(
    /// The whole Send flow: three modes, the step machine, EIP-681 locked
    /// requests, Max/fiat math, the treasury pre-check and the sign→submit
    /// lifecycle behind a single-flight re-entry lock.
    SendCore,
    vela_core::app::send::Send
);

bridge_class!(
    /// The activity feed: dedupe, batch folding, tombstones, celebrations.
    ActivityFeedCore,
    vela_core::app::activity_feed::ActivityFeed
);

bridge_class!(
    /// Manual custom-token management.
    ManageTokensCore,
    vela_core::app::manage_tokens::ManageTokens
);

bridge_class!(
    /// Browser history policy.
    BrowserHistoryCore,
    vela_core::app::browser_history::BrowserHistory
);

bridge_class!(
    /// Safari extension account snapshot + Universal Link TTL.
    ExtCacheCore,
    vela_core::app::ext_cache::ExtCache
);

bridge_class!(
    /// dApp connection lifecycle: pairing, fingerprint confirmation,
    /// reconnect policy and the timer discipline behind it.
    DappSessionCore,
    vela_core::app::dapp_session::DappSession
);
