package app.getvela.wallet.feature.multisig.core

import app.getvela.wallet.core.crux.Wire
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import uniffi.vela_core_uniffi.addressEntry
import uniffi.vela_core_uniffi.multisigCall
import uniffi.vela_core_uniffi.multisigLinkCall

/**
 * The multisig's pure rules, asked one at a time — through the link door
 * (`multisigLinkCall`, the web's `multisigLinkCall`: the link codec, row
 * ids, amounts in base units, member labels and kinds, the order networks
 * are offered in, the relay's token schedule) and the kernel door
 * (`multisigCall`: `validName`, `validOwner`, …). Nothing here decides: each
 * is one question to the core (iOS's `MultisigRules.swift`).
 */
object MsRules {
    /** One rule: [method] with its arguments, the answer as JSON (`null` when the core refused the question). */
    fun call(method: String, args: Map<String, Any?> = emptyMap()): JsonElement? =
        runCatching { Wire.json.parseToJsonElement(multisigLinkCall(method, jsonOf(args).toString())) }
            .getOrNull()?.takeUnless { it is JsonNull }

    fun string(method: String, args: Map<String, Any?> = emptyMap()): String? = call(method, args).str

    /** The kernel's answer (`multisigCall`). */
    fun kernel(method: String, args: Map<String, Any?> = emptyMap()): JsonElement? =
        runCatching { Wire.json.parseToJsonElement(multisigCall(method, jsonOf(args).toString())) }
            .getOrNull()?.takeUnless { it is JsonNull }

    // -- amounts --------------------------------------------------------------

    /** Base units as the page says them (`1,234.5`; `<0.000001`). */
    fun formatUnits(amount: String, decimals: Int): String =
        string("formatUnits", mapOf("amount" to amount, "decimals" to decimals)) ?: amount

    /** Base units as a plain decimal string (a record's `value`). */
    fun plainUnits(amount: String, decimals: Int): String =
        string("plainUnits", mapOf("amount" to amount, "decimals" to decimals)) ?: amount

    /** `1.5` → base units, or `null` when the text is not an amount. */
    fun parseUnits(text: String, decimals: Int): String? = call("parseUnits", mapOf("text" to text, "decimals" to decimals)).str

    // -- rows and links -------------------------------------------------------

    fun executedRowId(chainId: Int, safeTxHash: String, index: Int): String =
        string("executedRowId", mapOf("chainId" to chainId, "safeTxHash" to safeTxHash, "index" to index))
            ?: "multisig:$chainId:${safeTxHash.lowercase()}:$index"

    fun waitingRowId(chainId: Int, safeTxHash: String): String =
        string("waitingRowId", mapOf("chainId" to chainId, "safeTxHash" to safeTxHash))
            ?: "multisig:$chainId:${safeTxHash.lowercase()}"

    /** The multisig transaction an Activity row is, from its id. */
    fun transactionOfRow(id: String): Pair<Int, String>? {
        val answer = call("transactionOfRow", mapOf("id" to id)) ?: return null
        val chainId = answer["chainId"].int ?: return null
        val hash = answer["safeTxHash"].str ?: return null
        return chainId to hash
    }

    /** The link that opens a proposal on another member's device. */
    fun proposalLink(origin: String, locale: String, chainId: Int, multisig: String, safeTxHash: String): String? =
        string(
            "proposalLink",
            mapOf(
                "origin" to origin, "locale" to locale,
                "link" to mapOf("chainId" to chainId, "multisig" to multisig, "safeTxHash" to safeTxHash),
            ),
        )

    /** The invite link: the multisig, its networks, its members as this device names them. */
    fun inviteLink(
        origin: String,
        locale: String,
        address: String,
        owners: List<String>,
        chainIds: List<Int>,
        now: List<String>,
        known: Map<String, Any?>,
    ): String? = string(
        "inviteLink",
        mapOf(
            "origin" to origin, "locale" to locale,
            "account" to mapOf("address" to address, "owners" to owners, "chainIds" to chainIds),
            "now" to now, "known" to known,
        ),
    )

    /** r9-12: an invite from the core's own members (`AccountRow.invite_members`). */
    fun addLink(origin: String, locale: String, multisig: String, chainIds: List<Int>, members: List<JsonElement>): String? = string(
        "addLink",
        mapOf(
            "origin" to origin, "locale" to locale, "multisig" to multisig,
            "extra" to mapOf("chainIds" to chainIds, "members" to kotlinx.serialization.json.JsonArray(members)),
        ),
    )

    /** The connect link an outside owner opens in a browser with its wallet. */
    fun connectLink(page: String, invite: String?, members: List<String>, tx: Triple<Int, String, String>?): String? =
        string(
            "connectLink",
            mapOf(
                "page" to page, "invite" to invite, "members" to members,
                "tx" to tx?.let { mapOf("chainId" to it.first, "multisig" to it.second, "safeTxHash" to it.third) },
            ),
        )

    /** Where links point: the wallet's public web address (`shareOrigin`). */
    fun shareOrigin(): String = string("shareOrigin", mapOf("origin" to "app://android")) ?: "https://getvela.app"

    /** The address in its EIP-55 case (the core's `checksumAddress`). */
    fun eip55(address: String): String = runCatching { uniffi.vela_core_uniffi.checksumAddress(address) }.getOrDefault(address)

    val memberNameMax: Int get() = call("memberNameMax").int ?: 32

    /** Whether [hash] (a URL fragment, with or without `#`) is a multisig link. */
    fun isMultisigLink(hash: String): Boolean = call("isMultisigLink", mapOf("hash" to hash)).bool ?: false

    // -- names and networks (`multisig_rules`) --------------------------------

    /** The order networks are offered in for a new multisig, the default first. */
    fun likelyNetworks(networks: List<Int>, hints: Map<String, Any?>): List<Int> =
        call("networks.likely", mapOf("networks" to networks, "hints" to hints)).arr.mapNotNull { it.int }

    fun firstDefaultNetwork(networks: List<Int>, hints: Map<String, Any?>): Int? =
        call("networks.firstDefault", mapOf("networks" to networks, "hints" to hints)).int

    fun lastUsedNetworks(accounts: List<Map<String, Any?>>): JsonElement? =
        call("networks.lastUsed", mapOf("accounts" to accounts))

    /** core-ms-2: the core's catalog name for [chainId] (`null`: the catalog does not know it). */
    fun networkName(chainId: Int): String? = call("networks.name", mapOf("chainId" to chainId)).str

    // -- the kernel -----------------------------------------------------------

    fun validName(name: String): Boolean = kernel("validName", mapOf("name" to name)).bool ?: false

    fun validOwner(address: String): Boolean = kernel("validOwner", mapOf("address" to address)).bool ?: false

    /** `valid`, `malformed` or `checksum` (mixed case that is not its EIP-55). */
    fun addressEntryOf(address: String): String = runCatching { addressEntry(address) }.getOrDefault("malformed")
}
