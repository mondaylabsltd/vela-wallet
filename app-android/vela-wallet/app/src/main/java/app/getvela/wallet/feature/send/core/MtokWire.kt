package app.getvela.wallet.feature.send.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `manage_tokens` machine's wire, transcribed from
 * `rust/crates/vela-core/src/app/manage_tokens.rs` (spec 043).
 *
 * Adding a token by address: the core validates the address, asks for its
 * metadata on each candidate network, and decides what to save. The shell
 * does one multicall and one store write per request.
 *
 * Numerics: `chain_id` `u32` → `Int`; `decimals` `u8` → `Int`.
 */

// -- value types -------------------------------------------------------------

@Serializable
data class MtokTokenMeta(val name: String, val symbol: String, val decimals: Int)

@Serializable
data class MtokCustomToken(
    /** `"{chainId}_{contractAddress}"` — the same identity `token_trust` keys on. */
    val id: String,
    val chain_id: Int,
    val contract_address: String,
    val symbol: String,
    val name: String,
    val decimals: Int,
    val network_name: String,
)

@Serializable
data class MtokFound(
    val chain_id: Int,
    val network_name: String,
    val name: String,
    val symbol: String,
    val decimals: Int,
    val added: Boolean = false,
)

@Serializable
data class MtokNetwork(val chain_id: Int, val name: String)

@Serializable
data class MtokView(
    val input_address: String = "",
    val address_valid: Boolean = false,
    val detecting: Boolean = false,
    val found: List<MtokFound> = emptyList(),
    val saving: Boolean = false,
    val custom_tokens: List<MtokCustomToken> = emptyList(),
    val not_found: Boolean = false,
    /** The address is a network's native coin behind an ERC-20 interface (spec 060). */
    val native_alias: Boolean = false,
    val save_error: Boolean = false,
)

// -- what the machine asks for -----------------------------------------------

@Serializable
sealed class MtokOperation {
    @Serializable
    @SerialName("multicall_erc20_meta")
    data class MulticallErc20Meta(val chain_id: Int, val address: String) : MtokOperation()

    @Serializable
    @SerialName("read_custom_tokens")
    data object ReadCustomTokens : MtokOperation()

    @Serializable
    @SerialName("write_custom_token")
    data class WriteCustomToken(val token: MtokCustomToken) : MtokOperation()

    @Serializable
    @SerialName("remove_custom_token")
    data class RemoveCustomToken(val id: String) : MtokOperation()

    @Serializable
    @SerialName("invalidate_token_cache")
    data object InvalidateTokenCache : MtokOperation()
}

// -- what the shell observed -------------------------------------------------

@Serializable
sealed class MtokShellResult {
    /** `meta = null` = no ERC-20 answered at that address on that chain. */
    @Serializable
    @SerialName("chain_meta_resolved")
    data class ChainMetaResolved(
        val chain_id: Int,
        val address: String,
        val meta: MtokTokenMeta? = null,
    ) : MtokShellResult()

    @Serializable
    @SerialName("custom_tokens_loaded")
    data class CustomTokensLoaded(val tokens: List<MtokCustomToken> = emptyList()) : MtokShellResult()

    @Serializable
    @SerialName("saved")
    data object Saved : MtokShellResult()

    @Serializable
    @SerialName("save_failed")
    data object SaveFailed : MtokShellResult()

    @Serializable
    @SerialName("removed")
    data class Removed(val id: String) : MtokShellResult()

    @Serializable
    @SerialName("remove_failed")
    data class RemoveFailed(val id: String) : MtokShellResult()

    @Serializable
    @SerialName("cache_invalidated")
    data object CacheInvalidated : MtokShellResult()
}

// -- what the shell tells it -------------------------------------------------

@Serializable
sealed class MtokEvent {
    @Serializable
    @SerialName("start")
    data object Start : MtokEvent()

    @Serializable
    @SerialName("address_input")
    data class AddressInput(val s: String) : MtokEvent()

    @Serializable
    @SerialName("detect_requested")
    data class DetectRequested(val networks: List<MtokNetwork>) : MtokEvent()

    @Serializable
    @SerialName("save_requested")
    data class SaveRequested(val chain_id: Int) : MtokEvent()

    @Serializable
    @SerialName("delete_requested")
    data class DeleteRequested(val id: String) : MtokEvent()
}
