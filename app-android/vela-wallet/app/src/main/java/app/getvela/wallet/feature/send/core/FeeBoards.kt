package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import uniffi.vela_core_uniffi.FeePolicyCore

/**
 * The fee failures the gallery boards draw (PR 2 note 1), each a view the
 * REAL `fee_policy` machine wrote — a question asked, its account read
 * answered as the case says, its timers left running — so a board shows what
 * the core says about a failure (the row's figure and reason, whether it
 * retries by itself, the footer's line), not a drawing of it. Never on a live
 * surface.
 */
object FeeBoards {
    enum class Case {
        /** The chain's nodes did not answer the account read: the core retries by itself. */
        ChainDown,

        /** The read never left the app: retried by itself too, worded as Vela's own fault. */
        Internal,

        /** [ChainDown], and the core's own re-ask is out now: the reason stays, the sign turns. */
        Retrying,

        /** An undeployed account with no key to price it with: only a tap asks again. */
        TapOnly,
    }

    /** The view, as the core writes it, for [case] — a transfer on [chainId] from [account]. */
    fun view(case: Case, chainId: Int, account: String): FeeView =
        Wire.json.decodeFromString(FeeView.serializer(), viewJson(case, chainId, account))

    /** [view]'s JSON exactly as the core wrote it — what `signConfirmState` reads back. */
    fun viewJson(case: Case, chainId: Int, account: String): String {
        val read = when (case) {
            Case.Internal -> DeploymentRead.Internal(kind = "board")
            else -> DeploymentRead.Unreachable(rate_limited = false)
        }
        var reads = 0
        val script = CoreScript(FeePolicyCore().asBridge()) { operation ->
            when {
                // The first read only: a re-ask's own read stays out.
                operation.optString("type") == "read_deployment" && reads++ == 0 ->
                    Wire.json.encodeToString(FeeShellResult.serializer(), FeeShellResult.Deployment(read))
                // The run's deadline and the core's re-ask timer stay out:
                // the board is the moment the failure is on screen.
                else -> null
            }
        }
        val question = FeeEvent.QuoteRequested(
            chain_id = chainId,
            account = account,
            deployed = false,
            public_key_available = case != Case.TapOnly,
            tier = FeeTier.Standard,
            calls = listOf(FeeCall(to = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141", value = "1000000000000000")),
            read_deployment = case != Case.TapOnly,
        )
        script.dispatch(Wire.json.encodeToString(FeeEvent.serializer(), question))
        if (case == Case.Retrying) {
            // The core's own timer ran out: its re-ask goes, and its read is
            // left out — the moment the row keeps its reason and turns.
            script.resolve("start_ttl", Wire.json.encodeToString(FeeShellResult.serializer(), FeeShellResult.TtlElapsed))
        }
        return script.viewJson()
    }
}
