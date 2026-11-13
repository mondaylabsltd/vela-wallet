package app.getvela.wallet.core.crux

import uniffi.vela_core_uniffi.ActivityFeedCore
import uniffi.vela_core_uniffi.BalanceDashboardCore
import uniffi.vela_core_uniffi.ContactsCore
import uniffi.vela_core_uniffi.CreateWalletCore
import uniffi.vela_core_uniffi.DisplayCurrencyCore
import uniffi.vela_core_uniffi.LoginCore
import uniffi.vela_core_uniffi.FeePolicyCore
import uniffi.vela_core_uniffi.FeeSpeedCore
import uniffi.vela_core_uniffi.FeeTierPrefCore
import uniffi.vela_core_uniffi.SigningPagesCore
import uniffi.vela_core_uniffi.ManageTokensCore
import uniffi.vela_core_uniffi.ApprovalGuardCore
import uniffi.vela_core_uniffi.BatchImportCore
import uniffi.vela_core_uniffi.ClearSigningCore
import uniffi.vela_core_uniffi.SignRequestCore
import uniffi.vela_core_uniffi.BrowserHistoryCore
import uniffi.vela_core_uniffi.ExploreSitesCore
import uniffi.vela_core_uniffi.DappBrowserCore
import uniffi.vela_core_uniffi.DappPermissionsCore
import uniffi.vela_core_uniffi.SendCore
import uniffi.vela_core_uniffi.TxTrackerCore
import uniffi.vela_core_uniffi.NetworkAdminCore
import uniffi.vela_core_uniffi.PaymentRequestCore
import uniffi.vela_core_uniffi.ReceiveWatchCore
import uniffi.vela_core_uniffi.RpcPoolCore
import uniffi.vela_core_uniffi.SessionCore
import uniffi.vela_core_uniffi.TokenTrustCore
import uniffi.vela_core_uniffi.MultisigAccountsCore
import uniffi.vela_core_uniffi.MultisigMembersCore
import uniffi.vela_core_uniffi.MultisigOutsidePageCore
import uniffi.vela_core_uniffi.MultisigOutsideSessionCore
import uniffi.vela_core_uniffi.MultisigProposalsCore

/**
 * The three bridge methods, without caring which machine is behind them.
 *
 * uniffi generates one class per exported object with **no shared supertype**,
 * so this interface is what lets [CoreDriver] be written once instead of once
 * per machine. It is the app's entire binding surface to `vela-core`.
 */
interface CoreBridge {
    fun dispatch(eventJson: String): String
    fun resolveEffect(effectId: ULong, resultJson: String): String
    fun view(): String

    /**
     * Opt in to lighter answers (`vela_core::app::view_host`): from the next
     * answer on, `view_version` always, `view` only when it differs from the
     * last one sent, and — [includeVolatile] — the machine's volatile fields
     * apart in `volatile`. [CoreDriver] calls it before anything else and
     * reads both shapes. A bridge without the knob (a test's fake) keeps the
     * classic answers, which the driver reads as before.
     */
    fun skipUnchangedViews(includeVolatile: Boolean) {}
}

/**
 * Adapt one uniffi object, given its three methods.
 *
 * Every adapter below is the same five lines, and five lines repeated per
 * machine is five lines that can be got subtly wrong twenty-four times — the
 * kind of copy where `resolveEffect` ends up calling `dispatch` and the mistake
 * only shows as a machine that never settles. Bound references keep each
 * adapter to one line, so there is nothing left in it to get wrong.
 */
private fun bridgeOf(
    dispatch: (String) -> String,
    resolveEffect: (ULong, String) -> String,
    view: () -> String,
    skipUnchangedViews: (Boolean) -> Unit,
): CoreBridge = object : CoreBridge {
    override fun dispatch(eventJson: String) = dispatch.invoke(eventJson)
    override fun resolveEffect(effectId: ULong, resultJson: String) =
        resolveEffect.invoke(effectId, resultJson)
    override fun view() = view.invoke()
    override fun skipUnchangedViews(includeVolatile: Boolean) = skipUnchangedViews.invoke(includeVolatile)
}

// -- the machines this client drives ----------------------------------------
// One line per machine, matching one `bridge_object!` line in
// `rust/crates/vela-core-uniffi/src/onboarding_bridge.rs`.

/** Onboarding (spec 019). */
fun CreateWalletCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun LoginCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun SessionCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

/** Wallet state (spec 040). */
fun ContactsCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun NetworkAdminCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun DisplayCurrencyCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

/** The read path (spec 041). `RpcPoolCore` is what every other one reads through. */
fun RpcPoolCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun BalanceDashboardCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun ActivityFeedCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun ManageTokensCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

/** The in-app browser's decision half (spec 070). */
fun DappBrowserCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun DappPermissionsCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun ExploreSitesCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun BrowserHistoryCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun SignRequestCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun ClearSigningCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun ApprovalGuardCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

// Spec 045: the payroll batch the split rows are seeded from.
fun BatchImportCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

// Spec 043: the send path's three machines.
fun SendCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun FeePolicyCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun TxTrackerCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun TokenTrustCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun ReceiveWatchCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun PaymentRequestCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

// Spec 069: the default transaction speed, and the send screen's speed control.
fun FeeTierPrefCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun FeeSpeedCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

// Spec 102: Settings → Signing pages.
fun SigningPagesCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

// Native multisig (M1–M6): the five machines of `multisig_bridge.rs`, the
// same JSON surface the web drives — one line each, as every machine above.
fun MultisigAccountsCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun MultisigProposalsCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun MultisigMembersCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun MultisigOutsidePageCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)

fun MultisigOutsideSessionCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view, this::skipUnchangedViews)
