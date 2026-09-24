//! Emit the TypeScript mirrors of the wallet-state Core ↔ Shell wire types
//! (spec 016-crux-wallet-state).
//!
//! Same discipline as the onboarding generator: the boundary is JSON, so this
//! is what keeps both sides honest about every variant. Output is committed
//! and drift-gated.
//!
//! Run through `node rust/scripts/gen-core-types.mjs` (which adds the
//! `--check` gate), not directly.

use std::{env, error::Error, fs, path::PathBuf};

use ts_rs::{Config, TS};
use vela_core::app::activity_feed::{Event as FeedEvent, FeedOperation, FeedShellResult, FeedView};
use vela_core::app::approval_guard::{
    Event as GuardEvent, GuardOperation, GuardShellResult, GuardView,
};
use vela_core::app::balance_dashboard::{
    BalanceOperation, BalanceShellResult, BalanceView, Event as BalanceEvent,
};
use vela_core::app::batch_import::{
    BatchOperation, BatchShellResult, BatchView, Event as BatchImportEvent,
};
use vela_core::app::browser_history::{
    BhistOperation, BhistShellResult, BhistView, Event as BhistEvent,
};
use vela_core::app::clear_signing::{
    ClearOperation, ClearShellResult, ClearSigningView, Event as ClearSigningEvent,
};
use vela_core::app::contacts::{
    ContactOperation, ContactShellResult, ContactsView, Event as ContactEvent,
};
use vela_core::app::dapp_browser::{DbrOperation, DbrShellResult, DbrView, Event as DbrEvent};
use vela_core::app::dapp_permissions::{
    DpermOperation, DpermShellResult, DpermView, Event as DpermEvent,
};
use vela_core::app::dapp_session::{
    DsessOperation, DsessShellResult, DsessView, Event as DsessEvent,
};
use vela_core::app::display_currency::{
    CurrencyOperation, CurrencyShellResult, CurrencyView, Event as CurrencyEvent,
};
use vela_core::app::explore_sites::{
    Event as ExploreEvent, ExploreOperation, ExploreShellResult, ExploreView,
};
use vela_core::app::ext_cache::{
    Event as ExtCacheEvent, ExtCacheOperation, ExtCacheShellResult, ExtCacheView,
};
use vela_core::app::fee_policy::{Event as FeeEvent, FeeOperation, FeeShellResult, FeeView};
use vela_core::app::fee_speed::{
    Event as FeeSpeedEvent, FeeSpeedOperation, FeeSpeedShellResult, FeeSpeedView,
};
use vela_core::app::fee_tier_pref::{
    Event as FeeTierPrefEvent, FeeTierPrefOperation, FeeTierPrefShellResult, FeeTierPrefView,
};
use vela_core::app::manage_tokens::{Event as MtokEvent, MtokOperation, MtokShellResult, MtokView};
use vela_core::app::network_admin::{Event as NetEvent, NetOperation, NetShellResult, NetView};
use vela_core::app::payment_request::{
    Event as PaymentRequestEvent, PaymentRequestOperation, PaymentRequestShellResult,
    PaymentRequestView,
};
use vela_core::app::receive_watch::{
    Event as ReceiveWatchEvent, ReceiveWatchOperation, ReceiveWatchShellResult, ReceiveWatchView,
};
use vela_core::app::rpc_pool::{Event as RpcEvent, RpcOperation, RpcPoolView, RpcShellResult};
use vela_core::app::send::{Event as SendEvent, SendOperation, SendShellResult, SendView};
use vela_core::app::session::{
    Event as SessionEvent, SessionOperation, SessionShellResult, SessionView,
};
use vela_core::app::sign_pref::{
    Event as SignPrefEvent, SignPrefOperation, SignPrefShellResult, SignPrefView,
};
use vela_core::app::sign_request::{Event as SignEvent, SignOperation, SignShellResult, SignView};
use vela_core::app::token_trust::{
    Event as TrustEvent, TrustOperation, TrustShellResult, TrustView,
};
use vela_core::app::tx_tracker::{
    Event as TrackEvent, TrackOperation, TrackShellResult, TrackView,
};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = match env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
            .join("../../../app-web/vela-wallet/src/lib/core/generated"),
    };
    fs::create_dir_all(&out_dir)?;

    // `export_all` walks nested types, so these twelve roots cover the whole
    // surface: events in, operations out, results back, view models rendered.
    let config = Config::new().with_out_dir(&out_dir);
    CurrencyEvent::export_all(&config)?;
    CurrencyOperation::export_all(&config)?;
    CurrencyShellResult::export_all(&config)?;
    CurrencyView::export_all(&config)?;
    ReceiveWatchEvent::export_all(&config)?;
    ReceiveWatchOperation::export_all(&config)?;
    ReceiveWatchShellResult::export_all(&config)?;
    ReceiveWatchView::export_all(&config)?;
    PaymentRequestEvent::export_all(&config)?;
    PaymentRequestOperation::export_all(&config)?;
    PaymentRequestShellResult::export_all(&config)?;
    PaymentRequestView::export_all(&config)?;
    FeeEvent::export_all(&config)?;
    FeeOperation::export_all(&config)?;
    FeeShellResult::export_all(&config)?;
    FeeView::export_all(&config)?;
    FeeSpeedEvent::export_all(&config)?;
    FeeSpeedOperation::export_all(&config)?;
    FeeSpeedShellResult::export_all(&config)?;
    FeeSpeedView::export_all(&config)?;
    FeeTierPrefEvent::export_all(&config)?;
    FeeTierPrefOperation::export_all(&config)?;
    FeeTierPrefShellResult::export_all(&config)?;
    FeeTierPrefView::export_all(&config)?;
    SignPrefEvent::export_all(&config)?;
    SignPrefOperation::export_all(&config)?;
    SignPrefShellResult::export_all(&config)?;
    SignPrefView::export_all(&config)?;
    GuardEvent::export_all(&config)?;
    GuardOperation::export_all(&config)?;
    GuardShellResult::export_all(&config)?;
    GuardView::export_all(&config)?;
    ClearSigningEvent::export_all(&config)?;
    ClearOperation::export_all(&config)?;
    ClearShellResult::export_all(&config)?;
    ClearSigningView::export_all(&config)?;
    TrackEvent::export_all(&config)?;
    TrackOperation::export_all(&config)?;
    TrackShellResult::export_all(&config)?;
    TrackView::export_all(&config)?;
    ContactEvent::export_all(&config)?;
    ContactOperation::export_all(&config)?;
    ContactShellResult::export_all(&config)?;
    ContactsView::export_all(&config)?;
    BatchImportEvent::export_all(&config)?;
    BatchOperation::export_all(&config)?;
    BatchShellResult::export_all(&config)?;
    BatchView::export_all(&config)?;

    SessionEvent::export_all(&config)?;
    SessionOperation::export_all(&config)?;
    SessionShellResult::export_all(&config)?;
    SessionView::export_all(&config)?;
    BalanceEvent::export_all(&config)?;
    BalanceOperation::export_all(&config)?;
    BalanceShellResult::export_all(&config)?;
    BalanceView::export_all(&config)?;
    RpcEvent::export_all(&config)?;
    RpcOperation::export_all(&config)?;
    RpcShellResult::export_all(&config)?;
    RpcPoolView::export_all(&config)?;
    TrustEvent::export_all(&config)?;
    TrustOperation::export_all(&config)?;
    TrustShellResult::export_all(&config)?;
    TrustView::export_all(&config)?;
    NetEvent::export_all(&config)?;
    NetOperation::export_all(&config)?;
    NetShellResult::export_all(&config)?;
    NetView::export_all(&config)?;
    SignEvent::export_all(&config)?;
    SignOperation::export_all(&config)?;
    SignShellResult::export_all(&config)?;
    SignView::export_all(&config)?;
    DbrEvent::export_all(&config)?;
    DbrOperation::export_all(&config)?;
    DbrShellResult::export_all(&config)?;
    DbrView::export_all(&config)?;
    DpermEvent::export_all(&config)?;
    DpermOperation::export_all(&config)?;
    DpermShellResult::export_all(&config)?;
    DpermView::export_all(&config)?;
    FeedEvent::export_all(&config)?;
    FeedOperation::export_all(&config)?;
    FeedShellResult::export_all(&config)?;
    FeedView::export_all(&config)?;
    MtokEvent::export_all(&config)?;
    MtokOperation::export_all(&config)?;
    MtokShellResult::export_all(&config)?;
    MtokView::export_all(&config)?;
    BhistEvent::export_all(&config)?;
    BhistOperation::export_all(&config)?;
    BhistShellResult::export_all(&config)?;
    BhistView::export_all(&config)?;
    ExploreEvent::export_all(&config)?;
    ExploreOperation::export_all(&config)?;
    ExploreShellResult::export_all(&config)?;
    ExploreView::export_all(&config)?;
    ExtCacheEvent::export_all(&config)?;
    ExtCacheOperation::export_all(&config)?;
    ExtCacheShellResult::export_all(&config)?;
    ExtCacheView::export_all(&config)?;
    DsessEvent::export_all(&config)?;
    DsessOperation::export_all(&config)?;
    DsessShellResult::export_all(&config)?;
    DsessView::export_all(&config)?;
    SendEvent::export_all(&config)?;
    SendOperation::export_all(&config)?;
    SendShellResult::export_all(&config)?;
    SendView::export_all(&config)?;

    println!("wallet-state bindings written to {}", out_dir.display());
    Ok(())
}
