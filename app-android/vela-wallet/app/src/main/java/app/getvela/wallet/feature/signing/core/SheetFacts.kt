package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.core.crux.Wire
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.intOrNull

/**
 * The dApp request on the signing sheet as the agent's core reads it (spec
 * 103, `AgentShell.signing`): the machine's `SignRequestView`, with a `sheet`
 * member — what the sheet reads beside it (`agent_review::SheetFacts`). The
 * web's `sheetFacts` / `sheetBeside`: the clear-signing and guard views, and —
 * for THIS request only — its simulation and the fee in force, plus the
 * descriptor service a fetched reading names.
 *
 * The fee is what an agent's opinion is bound to: the core's
 * `AgentCore.advice` reads it from here, and a card whose fee moved past its
 * band is drawn as given at another fee. Pure, so every branch is tested on
 * the JVM.
 */
object SheetFacts {
    /**
     * The JSON [signJson]'s request is handed to the agent as, or `null` with
     * no request on the sheet. Each view is passed as its machine wrote it; one
     * that does not read is no reading, never the whole request lost.
     */
    fun signing(
        signJson: String?,
        clearJson: String?,
        guardJson: String?,
        feeJson: String?,
        sim: SigningController.SimOutcome?,
        simFacts: Pair<String, JsonElement>?,
        descriptorBase: String?,
    ): String? {
        val view = signJson?.let(::parsed) as? JsonObject ?: return null
        val request = view["request"] as? JsonObject ?: return null
        val id = (request["id"] as? JsonPrimitive)?.contentOrNull ?: return null
        val chainId = (request["chain_id"] as? JsonPrimitive)?.intOrNull
        val kind = (request["kind"] as? JsonPrimitive)?.contentOrNull
        val onChain = kind == "transaction" || kind == "batch"
        // A figure for another chain is a previous request's, not this one's.
        val fee = feeJson?.let(::parsed) as? JsonObject
        val feeChain = ((fee?.get("fee") as? JsonObject)?.get("chain_id") as? JsonPrimitive)?.intOrNull
        val feeMine = fee?.takeIf { feeChain == null || feeChain == chainId }
        // This request's own simulation, once it ended — and no other's.
        val ended = simFacts?.takeIf { it.first == id }?.second
        val sheet = buildJsonObject {
            put("clear", clearJson?.let(::parsed) ?: JsonNull)
            put("guard", guardJson?.let(::parsed) ?: JsonNull)
            put("fee", feeMine ?: JsonNull)
            put("simulation", ended ?: JsonNull)
            put("simulation_pending", JsonPrimitive(onChain && ended == null && sim is SigningController.SimOutcome.Pending))
            put("descriptor_base", JsonPrimitive(descriptorBase?.takeIf { it.isNotBlank() }))
        }
        return JsonObject(request + ("sheet" to sheet)).toString()
    }

    private fun parsed(text: String): JsonElement? = runCatching { Wire.json.parseToJsonElement(text) }.getOrNull()
}
