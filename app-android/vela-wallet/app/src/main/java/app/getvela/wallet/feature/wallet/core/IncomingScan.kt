package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.diagnostics.VelaLog
import java.math.BigDecimal
import java.math.BigInteger
import java.math.RoundingMode
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vela_core_uniffi.defaultMonitorChains
import org.json.JSONObject

/**
 * Money arriving: run one poll of the `token_trust` machine and persist what
 * it judged worth telling somebody about.
 *
 * Port of `syncReceivedTransfers` (`app-web/.../services/activity.ts`) plus the
 * `pollIncoming` driver (`token-trust-resident.ts`).
 *
 * **Two machines, one direction.** `token_trust` decides what counts as a
 * receipt; `activity_feed` decides how it reads. This class carries the answer
 * from one to the other and does not get a vote: it feeds the trust machine the
 * facts only a shell can know (which chains this person uses, what the registry
 * says, which tokens they already hold), waits for the poll to settle, and
 * writes the rows out.
 *
 * **A failed scan is a scan that found nothing.** Every path answers a count,
 * zero on any trouble — never an exception into the feed's executor, which
 * would leave an operation unanswered.
 */
class IncomingScan(
    private val trust: CoreHost<TrustView>,
    private val feed: FeedExecutor,
    private val chainInfo: suspend (Int) -> ChainInfo?,
    /**
     * The chains this person actually holds something on.
     *
     * Suspends, because it may have to wait for the first balance read. That
     * wait is the point: see [scan].
     */
    private val heldChains: suspend () -> List<Int>,
    /** Held ERC-20 contracts per chain — the trusted receive set. */
    private val heldTokens: (Int) -> List<String>,
    /** What a chain's native coin is called, for a native receipt's symbol. */
    private val nativeSymbol: (Int) -> String,
) {

    /** One poll at a time: a second while one runs would re-ask every chain. */
    private val polling = Mutex()

    suspend fun runOnce(address: String): Int {
        if (address.isBlank()) return 0
        if (!polling.tryLock()) return 0
        return try {
            scan(address)
        } catch (error: Throwable) {
            VelaLog.failure("feed.scan", "scan failed", error)
            0
        } finally {
            polling.unlock()
        }
    }

    private suspend fun scan(address: String): Int = coroutineScope {
        // The chains this wallet actually uses.
        //
        // **This waits, and the wait is load-bearing.** The feed's scan is
        // issued the moment an account is opened, which is the same moment the
        // balance read starts — so asking without waiting always found nothing
        // held, and fell back to the six default chains. Anybody whose money
        // sits on Optimism, Avalanche, Unichain, Monad or World Chain would
        // then have had no receipt monitoring at all on the first pass, and the
        // device log said `chains=6` without saying anything was wrong.
        //
        // The fallback is still there for its real purpose: a genuinely
        // brand-new wallet, whose very first receipt is the one that matters
        // most. The core owns that list; the shell borrows it because it must
        // fetch each chain's registry document before the poll can start.
        val held = heldChains().distinct()
        val chains = held.ifEmpty { defaultMonitorChains().map { it.toInt() } }

        // The registry's facts FIRST, and awaited: they are what the allowlist
        // is built from, and a poll that starts without them scans with an
        // empty allowlist and finds nothing.
        chains.map { chainId ->
            async {
                val info = chainInfo(chainId) ?: return@async
                trust.dispatch(
                    TrustEvent.RegistryTokensSnapshot(
                        chain_id = chainId,
                        stables = info.stables.map { it.contract },
                        wrapped_native = info.wrappedNative,
                    ),
                    TrustEvent.serializer(),
                )
            }
        }.awaitAll()

        chains.forEach { chainId ->
            val tokens = heldTokens(chainId)
            if (tokens.isNotEmpty()) {
                trust.dispatch(
                    TrustEvent.HeldTokensSnapshot(address, chainId, tokens),
                    TrustEvent.serializer(),
                )
            }
        }

        trust.dispatch(
            TrustEvent.HeldChainsSnapshot(address, chains),
            TrustEvent.serializer(),
        )
        trust.dispatch(TrustEvent.PollRequested(address), TrustEvent.serializer())

        // Settled, or the deadline.
        //
        // **Two phases, because dispatch is asynchronous.** Waiting only for
        // `!scanning` matches the state from BEFORE this poll began — the
        // machine has not been told to start yet, so it is trivially not
        // scanning. That answered instantly with the previous poll's feed and
        // reported `judged=0` while a real receipt was still being fetched, so
        // a payment would surface one poll late.
        //
        // Phase one waits for the poll to be visibly running; it is bounded,
        // because a poll the core declined (its own single-flight) never starts
        // one, and in that case reading what is already there is the right
        // answer rather than a twenty-second hang. Phase two waits for it to
        // finish, and returns at once if it already has.
        val settled = withTimeoutOrNull(POLL_DEADLINE_MS) {
            withTimeoutOrNull(POLL_START_MS) { trust.view.first { it.scanning } }
            trust.view.first { view -> !view.scanning && view.address != null }
        }
        val incoming = settled?.incoming.orEmpty()
        val records = incoming.mapNotNull { transfer -> record(transfer, address) }
        val stored = if (records.isEmpty()) 0 else feed.mergeRecords(records)

        // Logged BEFORE any early return, and on every path.
        //
        // The first version logged only when something landed, so a scan that
        // found nothing was indistinguishable from a scan that never ran — and
        // the first device run could not tell which. That is the same mistake
        // the balance executor made one phase earlier; here `timedOut` and the
        // gap between `judged` and `written` are the whole diagnosis.
        VelaLog.event(
            "feed.scan",
            "settled",
            "chains" to chains.size,
            "timedOut" to (settled == null),
            "judged" to incoming.size,
            "writable" to records.size,
            "new" to stored,
        )
        stored
    }

    /**
     * One judged transfer as a stored record.
     *
     * **A token whose metadata never resolved stays out.** Persisting it with a
     * fallback of 18 decimals would write a permanent, wrong "+0.000001" into
     * somebody's history for a receipt that might have been thousands. It is
     * retried on the next poll, while still inside the scan window.
     */
    private fun record(transfer: TrustIncomingView, address: String): JSONObject? {
        val symbol = when {
            transfer.is_native -> nativeSymbol(transfer.chain_id)
            else -> transfer.symbol ?: return null
        }
        val decimals = when {
            transfer.is_native -> NATIVE_DECIMALS
            else -> transfer.decimals ?: return null
        }

        val raw = runCatching { BigInteger(transfer.value) }.getOrNull() ?: return null
        val amount = BigDecimal(raw)
            .movePointLeft(decimals)
            .setScale(decimals.coerceAtMost(18), RoundingMode.DOWN)
            .stripTrailingZeros()
            .toPlainString()

        return JSONObject()
            .put("id", transfer.id)
            .put("userOpHash", "")
            .put("txHash", transfer.tx_hash)
            .put("from", transfer.from)
            .put("to", address)
            .put("value", amount)
            .put("symbol", symbol)
            .put("decimals", decimals)
            .put("chainId", transfer.chain_id)
            .put("timestamp", transfer.timestamp_sec)
            .put("status", "confirmed")
            .put("type", "receive")
            .apply {
                if (transfer.token != null) {
                    put("tokenAddress", transfer.token)
                }
            }
    }

    private companion object {
        const val POLL_DEADLINE_MS = 20_000L

        /**
         * How long to wait for a dispatched poll to become visible before
         * giving up on seeing it start. Generous enough for the core to be
         * driven on another thread, short enough that a declined poll costs
         * nothing.
         */
        const val POLL_START_MS = 3_000L
        const val NATIVE_DECIMALS = 18
    }
}
