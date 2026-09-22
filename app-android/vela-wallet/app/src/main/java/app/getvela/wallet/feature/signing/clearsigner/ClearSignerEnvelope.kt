package app.getvela.wallet.feature.signing.clearsigner

import org.json.JSONObject
import uniffi.vela_core_uniffi.ClearSignerOutcome
import uniffi.vela_core_uniffi.ClearSignerRefusal
import uniffi.vela_core_uniffi.clearSignerVerify
import uniffi.vela_core_uniffi.clearSignerVerifyCeremony

/**
 * PROTOCOL.md §4: the envelope a request rides in, and the sequence number
 * that keeps a replay out of it.
 *
 * The relay and BLE carry exactly the same JSON — only the wire under it
 * differs (a WebSocket frame there, six-byte-headed BLE frames here) — so the
 * shape lives once, and one channel cannot drift into a dialect of its own.
 *
 * `n` is taken as "the larger of what I have sent and what I have received,
 * plus one": each side's own numbers rise, the session's numbers rise, and a
 * receiver need only check that the other end's do.
 */
internal class ClearSignerEnvelopes {

    /** The highest `n` this session has sent or accepted. */
    var counter: Long = 0L
        private set

    /**
     * Accept the peer's `n`. `false` when it does not advance — a replayed or
     * reordered message, which is dropped rather than answered.
     */
    fun accept(n: Long): Boolean {
        if (n <= counter) return false
        counter = n
        return true
    }

    /** `{v:1,t:"intent",n,id,intent,context}` around the core's own request. */
    fun intent(id: String, requestJson: String): String {
        val request = runCatching { JSONObject(requestJson) }.getOrElse { JSONObject() }
        counter += 1
        return JSONObject()
            .put("v", 1)
            .put("t", "intent")
            .put("n", counter)
            .put("id", id)
            .put("intent", request.opt("intent") ?: JSONObject())
            .put("context", request.opt("context") ?: JSONObject())
            .toString()
    }

    /** `{v:1,t:"bye",n,reason:"done"}` — the wallet ending the session (§11). */
    fun bye(): String {
        counter += 1
        return JSONObject().put("v", 1).put("t", "bye").put("n", counter).put("reason", "done").toString()
    }
}

/**
 * The core judges, never a channel: a signature against the digest and the
 * wallet's keys, a ceremony against the operation's own rules and the page's
 * origin. [signerUrl] is the page the answer must have come from.
 */
internal fun judgeClearSignerAnswer(
    ask: ClearSignerAsk,
    answer: JSONObject,
    signerUrl: String,
): ClearSignerAnswer = when (ask) {
    is ClearSignerAsk.Signature -> {
        if (answer.optString("t") == "error") {
            ClearSignerAnswer.Signed(ClearSignerOutcome.Refused(clearSignerRefusalOf(answer.optString("code"))))
        } else {
            val result = answer.optJSONObject("result")
            if (result == null) {
                ClearSignerAnswer.Signed(
                    ClearSignerOutcome.Refused(ClearSignerRefusal.Malformed("no result")),
                )
            } else {
                ClearSignerAnswer.Signed(clearSignerVerify(result.toString(), ask.digest, ask.keys))
            }
        }
    }
    is ClearSignerAsk.Ceremony -> ClearSignerAnswer.Ceremonial(
        clearSignerVerifyCeremony(
            ask.operationJson,
            answer.toString(),
            signerUrl,
            ask.expectedMemberChallenge,
        ),
    )
}

/** `clear_signer::refusal` — the person, or the page's own rules. */
internal fun clearSignerRefusalOf(code: String): ClearSignerRefusal =
    if (code.isEmpty() || code == "user_rejected") {
        ClearSignerRefusal.Declined
    } else {
        ClearSignerRefusal.PageRefused(code)
    }
