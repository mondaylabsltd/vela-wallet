package app.getvela.wallet.feature.wallet.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The two machines behind the receive screen, transcribed from
 * `rust/crates/vela-core/src/app/receive_watch.rs` and `payment_request.rs`.
 *
 * `receive_watch` notices money arriving while somebody is looking at their own
 * QR code. `payment_request` builds the thing they are showing — the address,
 * or a request for a specific amount of a specific token.
 *
 * Numeric types from the Rust: `chain_id`, `decimals` and `ms` are `u32` →
 * `Int`; `balance`, `usd`, `amount` and every `_ms` are `f64` → `Double`.
 *
 * **An amount is a string on the way out.** `PayRequest.amount_base` is base
 * units as a decimal string and never a JSON number: base units routinely
 * exceed what a double can hold exactly, and this is the number a payment is
 * made from.
 */

// -- receive_watch -----------------------------------------------------------

/**
 * One token's balance, as the watcher compares it.
 *
 * `balance` is an f64 on purpose — the detection threshold has to be
 * bit-identical to the one every other client uses, or the same deposit is
 * noticed on one device and missed on another.
 */
@Serializable
data class TokenSnapshot(
    val id: String,
    val symbol: String,
    val chain_id: Int,
    val balance: Double,
    val price_usd: Double? = null,
)

@Serializable
sealed class ReceiveWatchOperation {
    /**
     * Refresh the balances, forced.
     *
     * The shell checks whether the app is actually in front of somebody FIRST
     * and answers `inactive` without fetching when it is not — a wallet that
     * polls a dozen chains from a pocket is a battery complaint.
     */
    @Serializable
    @SerialName("fetch_tokens")
    data object FetchTokens : ReceiveWatchOperation()

    /** Wait, so the core never owns a clock. */
    @Serializable
    @SerialName("wait")
    data class Wait(val ms: Int) : ReceiveWatchOperation()

    /** Money landed: buzz. */
    @Serializable
    @SerialName("signal_deposit")
    data object SignalDeposit : ReceiveWatchOperation()
}

@Serializable
sealed class ReceiveWatchShellResult {
    @Serializable
    @SerialName("tokens_fetched")
    data class TokensFetched(
        val tokens: List<TokenSnapshot> = emptyList(),
        val now_ms: Double,
    ) : ReceiveWatchShellResult()

    @Serializable
    @SerialName("fetch_failed")
    data class FetchFailed(val now_ms: Double) : ReceiveWatchShellResult()

    /** The app was backgrounded when the tick fired. */
    @Serializable
    @SerialName("inactive")
    data object Inactive : ReceiveWatchShellResult()

    @Serializable
    @SerialName("waited")
    data class Waited(val now_ms: Double) : ReceiveWatchShellResult()

    @Serializable
    @SerialName("signalled")
    data object Signalled : ReceiveWatchShellResult()
}

@Serializable
sealed class ReceiveWatchEvent {
    /**
     * The screen opened.
     *
     * One session per account: switching accounts disposes this machine and
     * builds a fresh one, so a previous account's baseline can never bleed
     * into the new one and be read as a deposit.
     */
    @Serializable
    @SerialName("start")
    data object Start : ReceiveWatchEvent()
}

@Serializable
data class DepositItem(
    val symbol: String,
    /** The raw balance delta; the shell formats it. */
    val amount: Double,
    val chain_id: Int,
    val usd: Double? = null,
)

@Serializable
data class DepositEntry(
    /** When the detecting fetch happened — the shell formats it per locale. */
    val at_epoch_ms: Double,
    val items: List<DepositItem> = emptyList(),
)

@Serializable
data class ReceiveWatchView(
    val detected: Boolean = false,
    /** Newest first. */
    val deposits: List<DepositEntry> = emptyList(),
)

// -- payment_request ---------------------------------------------------------

@Serializable
enum class ReceiveMode {
    @SerialName("address") Address,

    @SerialName("request") Request,
}

/** What a picked asset pins down. The picker's catalog stays in the shell. */
@Serializable
data class ReceiveAsset(
    val chain_id: Int = 1,
    val token_address: String? = null,
    val symbol: String = "ETH",
    val decimals: Int = 18,
    val network_name: String = "Ethereum",
)

/** A validated `/pay` request, every field already normalised by the core. */
@Serializable
data class PayRequest(
    val recipient: String,
    val chain_id: Int,
    val token_address: String? = null,
    /** The human amount as displayed, or `null` for an open request. */
    val amount: String? = null,
    /** Base units as a decimal STRING, present exactly when [amount] is. */
    val amount_base: String? = null,
    val symbol: String,
    val decimals: Int,
    val network_name: String,
    val eip681_uri: String,
)

@Serializable
sealed class PaymentRequestOperation {
    /** `vela.receiveWarned.{account}`. A read error answers `false` — show the gate. */
    @Serializable
    @SerialName("read_ack")
    data class ReadAck(val account: String) : PaymentRequestOperation()

    @Serializable
    @SerialName("write_ack")
    data class WriteAck(val account: String) : PaymentRequestOperation()
}

@Serializable
sealed class PaymentRequestShellResult {
    @Serializable
    @SerialName("ack_flag")
    data class AckFlag(val acknowledged: Boolean) : PaymentRequestShellResult()

    @Serializable
    @SerialName("ack_written")
    data object AckWritten : PaymentRequestShellResult()
}

@Serializable
sealed class PaymentRequestEvent {
    /**
     * The receive screen opened.
     *
     * `base_url` is where a pay link points. The core never reads a location:
     * on a phone there is no origin to read, so the shell supplies the public
     * one.
     */
    @Serializable
    @SerialName("start")
    data class Start(
        val account: String,
        val recipient: String,
        val base_url: String,
    ) : PaymentRequestEvent()

    @Serializable
    @SerialName("mode_changed")
    data class ModeChanged(val mode: ReceiveMode) : PaymentRequestEvent()

    @Serializable
    @SerialName("asset_picked")
    data class AssetPicked(
        val chain_id: Int,
        val token_address: String? = null,
        val symbol: String,
        val decimals: Int,
        val network_name: String,
    ) : PaymentRequestEvent()

    /** Already dot-normalised by the shell; the core applies the sanitise rules. */
    @Serializable
    @SerialName("amount_changed")
    data class AmountChanged(val text: String) : PaymentRequestEvent()

    @Serializable
    @SerialName("acknowledge")
    data object Acknowledge : PaymentRequestEvent()

    /** A `/pay` link's raw query — entirely untrusted, and parsed by the core. */
    @Serializable
    @SerialName("link_opened")
    data class LinkOpened(
        val to: String? = null,
        val chain: String? = null,
        val token: String? = null,
        val amount: String? = null,
        val sym: String? = null,
        val dec: String? = null,
        val net: String? = null,
    ) : PaymentRequestEvent()
}

@Serializable
data class PaymentRequestView(
    /** The QR stays covered while this is true, so a first visit never flashes it. */
    val gate_loading: Boolean = true,
    val acknowledged: Boolean = false,
    val can_copy: Boolean = false,
    val can_save: Boolean = false,
    val mode: ReceiveMode = ReceiveMode.Address,
    val asset: ReceiveAsset = ReceiveAsset(),
    val amount: String = "",
    val eip681_uri: String = "",
    val pay_link: String = "",
    /** What the QR encodes. */
    val qr_value: String = "",
    /** What the copy button copies — a pay link in request mode, else the address. */
    val copy_payload: String = "",
    val has_amount: Boolean = false,
    val pay_valid: Boolean? = null,
    val pay: PayRequest? = null,
)
