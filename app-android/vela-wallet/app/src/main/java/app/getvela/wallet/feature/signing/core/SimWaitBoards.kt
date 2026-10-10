package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.ApprovalGuardCore
import uniffi.vela_core_uniffi.ClearSigningCore
import uniffi.vela_core_uniffi.SignRequestCore

/**
 * The confirm's wait for the simulation's verdict (PR 3, fix C), at the two
 * moments a live sheet on a real phone rarely holds still on — each a set of
 * views the REAL machines wrote, for the gallery's boards (CS69, CS70) and
 * never for a live surface.
 *
 * A token transfer arrives from a site, as the sheet's controller announces
 * one: the networks, the account, the request, and then `sim_started` — "the
 * wallet's own simulation of this request is out". The sign machine starts
 * its deadline (`sim_verdict_timer`), and:
 *
 * - **held** — the deadline is left out, as on a sheet whose simulation has
 *   not answered and whose four seconds have not passed. The board stays
 *   held: no timer runs here at all.
 * - **waited out** — the deadline is answered (`sim_verdict_timer_fired`),
 *   as the sign executor answers it after its `ms`: the confirm opens and
 *   the view names the could-not-check sentence.
 *
 * The reading and the approval guard are the real machines' too, over the
 * same request, with nothing on the network answering — so "everything else
 * is ready" is what those machines say, and the gate that reads all of it is
 * the core's (`SigningLive.confirmState`), not a board's drawing of it.
 */
object SimWaitBoards {

    /** The views the confirm gate reads, exactly as the machines wrote them. */
    class Views(val signJson: String, val guardJson: String, val clearJson: String) {
        val sign: SignView get() = Wire.json.decodeFromString(SignView.serializer(), signJson)
        val guard: GuardView get() = Wire.json.decodeFromString(GuardView.serializer(), guardJson)
        val clear: ClearSigningView get() = Wire.json.decodeFromString(ClearSigningView.serializer(), clearJson)
    }

    const val REQUEST_ID = "board-sim-wait"
    private const val METHOD = "eth_sendTransaction"
    private const val ORIGIN = "https://app.uniswap.org"

    /** The boards' clock: 2026-10-11 00:00:00 UTC. Nothing drawn reads it. */
    private const val NOW_MS = 1_791_676_800_000.0

    /** USDC, and CS1's transfer of 1,000 of it. */
    private const val TOKEN = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
    private const val RECIPIENT = "af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1"

    /** `[{to, data, value}]` — `transfer(recipient, 1000 USDC)`. */
    fun params(): String = JSONArray().put(
        JSONObject()
            .put("to", TOKEN)
            .put("data", "0xa9059cbb" + RECIPIENT.padStart(64, '0') + 1_000_000_000L.toString(16).padStart(64, '0'))
            .put("value", "0x0"),
    ).toString()

    /**
     * The request on [chainId] signed by [account], its simulation announced,
     * and — [waitedOut] — its deadline passed with no verdict.
     */
    fun views(waitedOut: Boolean, account: String, chainId: Int = 1): Views {
        val wire = Wire.json
        val params = params()

        // -- sign_request: the request, and the wait ----------------------------
        var deadline: JSONObject? = null
        val sign = CoreScript(SignRequestCore().asBridge()) { operation ->
            when (operation.optString("type")) {
                // The deadline of the wait: kept out. Answered below, by hand,
                // only for the board that shows what follows it.
                "sim_verdict_timer" -> {
                    deadline = operation
                    null
                }
                "switch_active_account" -> wire.encodeToString(SignShellResult.serializer(), SignShellResult.AccountSwitched)
                else -> null
            }
        }
        fun tell(event: SignEvent) = sign.dispatch(wire.encodeToString(SignEvent.serializer(), event))
        tell(SignEvent.NetworksChanged(listOf(chainId)))
        tell(SignEvent.AccountsChanged(listOf(SignAccountRef(address = account, credential_id = "board")), 0))
        tell(
            SignEvent.RequestArrived(
                id = REQUEST_ID, method = METHOD, params_json = params, origin = ORIGIN, transport_id = "board",
                dedicated_transport = true, per_request_chain = chainId, granted_address = account, now_ms = NOW_MS,
            ),
        )
        // What `SigningController.open` says in the step that sends the simulation.
        tell(SignEvent.SimStarted(REQUEST_ID))
        if (waitedOut) {
            val asked = deadline ?: error("the sign machine started no deadline for the simulation's verdict")
            sign.resolve(
                "sim_verdict_timer",
                wire.encodeToString(
                    SignShellResult.serializer(),
                    SignShellResult.SimVerdictTimerFired(id = asked.optString("id"), round = asked.optInt("round")),
                ),
            )
        }

        // -- clear_signing: the reading, with nothing on the network answering --
        val clear = CoreScript(ClearSigningCore().asBridge()) { operation ->
            val answer: ClearShellResult = when (val asked = wire.decodeFromString(ClearOperation.serializer(), operation.toString())) {
                is ClearOperation.HttpGet -> ClearShellResult.DescriptorFetched(asked.path, null)
                is ClearOperation.RpcEthCall -> ClearShellResult.RpcAnswer(asked.probe, asked.chain_id, asked.to, null, false)
                is ClearOperation.SelectorDbLookup -> ClearShellResult.SelectorCandidates()
                is ClearOperation.Timer -> ClearShellResult.TimedOut(asked.token)
                ClearOperation.Now -> ClearShellResult.Clock(NOW_MS)
            }
            wire.encodeToString(ClearShellResult.serializer(), answer)
        }
        SigningController.clearKickoff(METHOD, params, chainId, ORIGIN)?.let { kickoff ->
            clear.dispatch(wire.encodeToString(ClearSigningEvent.serializer(), kickoff))
        }

        // -- approval_guard: a transfer grants nothing --------------------------
        val guard = CoreScript(ApprovalGuardCore().asBridge()) { operation ->
            val answer: GuardShellResult = when (wire.decodeFromString(GuardOperation.serializer(), operation.toString())) {
                is GuardOperation.ReadTokenMetadata -> GuardShellResult.MetaResolved(null)
                is GuardOperation.ReadErc20Allowance -> GuardShellResult.AllowanceRead(null)
                is GuardOperation.ReadErc20Balance -> GuardShellResult.BalanceRead(null)
            }
            wire.encodeToString(GuardShellResult.serializer(), answer)
        }
        val detected: GuardEvent = GuardEvent.ApprovalDetected(
            method = METHOD, params_json = params, chain_id = chainId, wallet_address = account, read_only = false, now_ms = NOW_MS,
        )
        guard.dispatch(wire.encodeToString(GuardEvent.serializer(), detected))

        return Views(signJson = sign.viewJson(), guardJson = guard.viewJson(), clearJson = clear.viewJson())
    }
}
