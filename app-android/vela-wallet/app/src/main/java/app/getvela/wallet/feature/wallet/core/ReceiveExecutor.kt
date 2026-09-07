package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.delay

/**
 * The two shells behind the receive screen.
 *
 * `receive_watch` has three operations and `payment_request` has two, and
 * between them they hold no judgement at all: what counts as a deposit, how
 * often to look, when to give up, what a pay link says, and whether an amount
 * is valid are `receive_watch.rs` and `payment_request.rs`.
 *
 * Port source: `app-web/vela-wallet/src/lib/wallet/core/`.
 */
class ReceiveExecutor(
    private val store: KeyValueStore,
    /** This account's balances, as the watcher compares them. */
    private val snapshot: suspend () -> List<TokenSnapshot>?,
    /** Is the app actually in front of somebody right now? */
    private val foreground: () -> Boolean = { true },
    private val haptic: () -> Unit = {},
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
) {

    // -- receive_watch --------------------------------------------------------

    suspend fun performWatch(operation: ReceiveWatchOperation): ReceiveWatchShellResult =
        when (operation) {

            /*
             * **The activity check comes first, and it is not an optimisation.**
             *
             * A backgrounded wallet that keeps polling a dozen chains every few
             * seconds is a battery complaint, and worse, it is one nobody can
             * attribute. The core stops the session when it hears `inactive`,
             * so answering this honestly is what ends the polling.
             */
            is ReceiveWatchOperation.FetchTokens -> when {
                !foreground() -> ReceiveWatchShellResult.Inactive
                else -> {
                    val tokens = runCatching { snapshot() }.getOrNull()
                    if (tokens == null) {
                        ReceiveWatchShellResult.FetchFailed(now_ms = now())
                    } else {
                        ReceiveWatchShellResult.TokensFetched(tokens = tokens, now_ms = now())
                    }
                }
            }

            is ReceiveWatchOperation.Wait -> {
                delay(operation.ms.toLong())
                ReceiveWatchShellResult.Waited(now_ms = now())
            }

            is ReceiveWatchOperation.SignalDeposit -> {
                haptic()
                VelaLog.event("receive.watch", "deposit signalled")
                ReceiveWatchShellResult.Signalled
            }
        }

    fun neutralWatchAnswer(operation: ReceiveWatchOperation): ReceiveWatchShellResult =
        when (operation) {
            // A failed fetch, not a settled empty one: `tokens_fetched` with
            // nothing in it would read as "every balance went to zero", which
            // the core would report as a very large withdrawal.
            is ReceiveWatchOperation.FetchTokens -> ReceiveWatchShellResult.FetchFailed(now())
            is ReceiveWatchOperation.Wait -> ReceiveWatchShellResult.Waited(now())
            is ReceiveWatchOperation.SignalDeposit -> ReceiveWatchShellResult.Signalled
        }

    // -- payment_request ------------------------------------------------------

    suspend fun performRequest(
        operation: PaymentRequestOperation,
    ): PaymentRequestShellResult = when (operation) {

        // A read error answers `false`, which SHOWS the gate. The failure mode
        // that matters is the other one: a wallet that decides somebody has
        // already seen the warning because storage was slow.
        is PaymentRequestOperation.ReadAck -> PaymentRequestShellResult.AckFlag(
            acknowledged = store.read(ackKey(operation.account)) == "1",
        )

        is PaymentRequestOperation.WriteAck -> {
            store.write(ackKey(operation.account), "1")
            PaymentRequestShellResult.AckWritten
        }
    }

    fun neutralRequestAnswer(
        operation: PaymentRequestOperation,
    ): PaymentRequestShellResult = when (operation) {
        is PaymentRequestOperation.ReadAck -> PaymentRequestShellResult.AckFlag(false)
        is PaymentRequestOperation.WriteAck -> PaymentRequestShellResult.AckWritten
    }

    /**
     * `vela.receiveWarned.{account}` — the key every Vela client uses.
     *
     * Per account, not per device: the warning is about a particular address's
     * receiving rules, and somebody who has two wallets has seen it for one of
     * them.
     */
    private fun ackKey(account: String) = "vela.receiveWarned.$account"
}
