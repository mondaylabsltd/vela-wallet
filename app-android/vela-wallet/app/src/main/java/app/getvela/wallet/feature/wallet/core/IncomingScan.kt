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
    /**
     * r9-17 (core §9.4): the rest of the facts the core plans a scan from —
     * the networks [address] lives on (a multisig's), the space in front's
     * networks in order, the networks the person added.
     */
    private val accountNetworks: (address: String) -> List<Int> = { emptyList() },
    private val spaceChainIds: () -> List<Int> = { emptyList() },
    private val addedChainIds: () -> List<Int> = { emptyList() },
    /** Where an account's scan coverage is kept (`vela.scanCursors.<address>`), across restarts. */
    private val store: app.getvela.wallet.core.data.KeyValueStore? = null,
) {

    /** One poll at a time: a second while one runs would re-ask every chain. */
    private val polling = Mutex()

    /** One poll's answer: how many receipts were new, and the networks it could not read. */
    data class Outcome(val newCount: Int, val unreadChains: List<Int>)

    /** How many receipts one poll found new ([read]'s count). */
    suspend fun runOnce(address: String): Int = read(address).newCount

    /**
     * One poll: how many receipts were new, and (round 16 leftover 5) the
     * networks of [address] it could not read — the trust machine's
     * `unread_chains` once the poll settled. A poll that did not run (one is
     * already out, or it threw) says what the machine last knew of this
     * account, never an empty list it did not read.
     */
    suspend fun read(address: String): Outcome {
        if (address.isBlank()) return Outcome(0, emptyList())
        if (!polling.tryLock()) return Outcome(0, unreadOf(trust.view.value, address))
        return try {
            scan(address)
        } catch (error: Throwable) {
            VelaLog.failure("feed.scan", "scan failed", error)
            Outcome(0, unreadOf(trust.view.value, address))
        } finally {
            polling.unlock()
        }
    }

    /** The networks the trust machine could not read for [address] (`unread_chains`), ascending; empty for another account. */
    private fun unreadOf(view: TrustView, address: String): List<Int> =
        if (view.address.equals(address, ignoreCase = true)) view.unread_chains.distinct().sorted() else emptyList()

    private suspend fun scan(address: String): Outcome = coroutineScope {
        // The chains this wallet holds something on. **This waits, and the
        // wait is load-bearing**: the feed's scan is issued the moment an
        // account is opened, which is when the balance read starts — asked
        // without waiting it always found nothing held.
        val held = heldChains().distinct()
        // r9-17 (core §9.4): the networks, the windows, the backfill and the
        // cadence are the core's. This shell hands it facts — which networks
        // the account holds something on, lives on, the space's and the
        // person's own — and the coverage it kept; it no longer picks the
        // chains itself (it fell back to mainnet chains only, so a multisig on
        // Arc Testnet never heard of money arriving).
        val facts = TrustScanFacts(
            held_chain_ids = held,
            account_networks = accountNetworks(address).distinct().map { TrustAccountNetwork(it) },
            space_chain_ids = spaceChainIds(),
            added_chain_ids = addedChainIds(),
        )
        val plan = runCatching {
            org.json.JSONObject(uniffi.vela_core_uniffi.incomingScanPlan(app.getvela.wallet.core.crux.Wire.json.encodeToString(TrustScanFacts.serializer(), facts)))
                .optJSONArray("scan_chain_ids")
        }.getOrNull()
        val chains = (0 until (plan?.length() ?: 0)).map { plan!!.getInt(it) }

        // The registry's facts FIRST, and awaited, for every network the core
        // will scan: the allowlist is built from them, and a poll that starts
        // without them finds nothing.
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

        trust.dispatch(TrustEvent.ScanRequested(address, facts, readCursors(address)), TrustEvent.serializer())

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
        // What this poll covered, kept for the next (a restart resumes the backfill).
        settled?.takeIf { it.address.equals(address, ignoreCase = true) }?.cursors?.let { writeCursors(address, it) }
        // Round 16 leftover 5: the networks this poll could not read, as the
        // machine says once it settled (the deadline: what it says now).
        val unread = unreadOf(settled ?: trust.view.value, address)
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
            "unread" to unread.size,
        )
        Outcome(stored, unread)
    }

    private suspend fun readCursors(address: String): List<TrustScanCursor> {
        val raw = store?.read(CURSOR_KEY_PREFIX + address.lowercase()) ?: return emptyList()
        return runCatching {
            app.getvela.wallet.core.crux.Wire.json.decodeFromString(kotlinx.serialization.builtins.ListSerializer(TrustScanCursor.serializer()), raw)
        }.getOrDefault(emptyList())
    }

    private suspend fun writeCursors(address: String, cursors: List<TrustScanCursor>) {
        if (cursors.isEmpty()) return
        store?.write(
            CURSOR_KEY_PREFIX + address.lowercase(),
            app.getvela.wallet.core.crux.Wire.json.encodeToString(kotlinx.serialization.builtins.ListSerializer(TrustScanCursor.serializer()), cursors),
        )
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
            // Its time is its block's own, by construction: the trust feed
            // holds no transfer whose block it has not read (PR 3). The mark
            // is what tells this record from one stored before that, when a
            // block that could not be read was stamped with the clock — those
            // the feed's core re-reads and rewrites (`FeedExecutor`).
            .put("timeVerified", true)
            .put("status", "confirmed")
            .put("type", "receive")
            .apply {
                if (transfer.token != null) {
                    put("tokenAddress", transfer.token)
                }
            }
    }

    private companion object {
        /** The core's key (`incoming_scan::CURSOR_KEY_PREFIX`) + the lowercase address. */
        const val CURSOR_KEY_PREFIX = "vela.scanCursors."
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
