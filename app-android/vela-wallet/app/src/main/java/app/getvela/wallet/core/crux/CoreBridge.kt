package app.getvela.wallet.core.crux

import uniffi.vela_core_uniffi.ContactsCore
import uniffi.vela_core_uniffi.CreateWalletCore
import uniffi.vela_core_uniffi.DisplayCurrencyCore
import uniffi.vela_core_uniffi.LoginCore
import uniffi.vela_core_uniffi.NetworkAdminCore
import uniffi.vela_core_uniffi.SessionCore

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
): CoreBridge = object : CoreBridge {
    override fun dispatch(eventJson: String) = dispatch.invoke(eventJson)
    override fun resolveEffect(effectId: ULong, resultJson: String) =
        resolveEffect.invoke(effectId, resultJson)
    override fun view() = view.invoke()
}

// -- the machines this client drives ----------------------------------------
// One line per machine, matching one `bridge_object!` line in
// `rust/crates/vela-core-uniffi/src/onboarding_bridge.rs`.

/** Onboarding (spec 019). */
fun CreateWalletCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view)

fun LoginCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view)

fun SessionCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view)

/** Wallet state (spec 040). */
fun ContactsCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view)

fun NetworkAdminCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view)

fun DisplayCurrencyCore.asBridge(): CoreBridge =
    bridgeOf(this::dispatch, this::resolveEffect, this::view)
