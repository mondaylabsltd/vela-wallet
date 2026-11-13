package app.getvela.wallet

import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.core.ConfirmState
import app.getvela.wallet.feature.signing.core.SigningController
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull

/**
 * The confirm's gate as a test reads it, and how a test approves.
 *
 * The gate is the core's (`signConfirmState`), over the four views exactly
 * as the machines last wrote them and the speed in force — what the live
 * sheet passes it (`VelaNavHost`). `SignView.confirm_gate_open` is only one
 * part of it: it knows nothing of the reading, the fee, or the simulation's
 * verdict the confirm has waited for since PR 3.
 *
 * The core does not take an approve sent while the gate is shut — on
 * purpose, and without a word: a tap from a stale frame waits too. A test
 * that sent one would then wait for a signature that is never coming. So a
 * test approves through [approveWhenOpen], and what it waits for is what a
 * person waits for.
 */
fun SigningController.confirmGate(): Flow<ConfirmState> =
    combine(combine(signJson, guardJson, clearJson, feeJson) { sign, guard, clear, fee -> listOf(sign, guard, clear, fee) }, speed, sim) { views, speed, sim ->
        // R22 / §41: the sheet's own simulation verdict goes to the gate too, as the live sheet passes it.
        SigningLive.confirmState(views[0], views[1], views[2], views[3], speed.tier, simJson = sim?.verdictJson)
    }

/** The gate, once [test] holds of it. A gate that never gets there fails with what it last said. */
suspend fun SigningController.awaitGate(what: String, timeoutMs: Long = 30_000, test: (ConfirmState) -> Boolean): ConfirmState {
    var last = ConfirmState()
    return withTimeoutOrNull(timeoutMs) {
        confirmGate().first {
            last = it
            test(it)
        }
    } ?: throw AssertionError("$what — the gate last said $last")
}

/**
 * The confirm, tapped as a person can tap it: once the gate is open — and,
 * §41 (P2-5), completed as the sheet asks: an "… anyway" confirm is the held
 * press, and the approve says so (`anyway_confirmed`), as the live sheet's does.
 */
suspend fun SigningController.approveWhenOpen(timeoutMs: Long = 30_000): ConfirmState {
    val open = awaitGate("the confirm never opened, so nothing was approved", timeoutMs) { it.enabled }
    approve(anywayConfirmed = open.anyway)
    return open
}
