package app.getvela.wallet.feature.multisig

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.multisig.core.MultisigHub
import kotlinx.serialization.json.JsonElement
import org.json.JSONObject

/**
 * r13-5 (r12-8, core §13.3): an executed member change's Activity row is
 * titled when it is DRAWN, never frozen in the words of the moment it was
 * written. The row keeps its action (`multisigChange`); whenever the
 * language or the members' names move, every such row is worded again by
 * the core's one rule (`names.change`, through [MultisigWords.memberChangeTitle])
 * — the web's and iOS's `rewordMemberRows`, the desktop's `reword_member_rows`.
 * Rows written before the action was kept carry none and keep their title.
 */
object MemberRowTitles {

    /**
     * Words [row] again with [title] (the action → its title, `null` when
     * the rule says nothing). Answers whether the row's title moved.
     */
    fun reword(row: JSONObject, title: (JsonElement) -> String?): Boolean {
        if (!row.optString("id").startsWith("multisig:")) return false
        val stored = row.optJSONObject("multisigChange") ?: return false
        val action = runCatching { Wire.json.parseToJsonElement(stored.toString()) }.getOrNull() ?: return false
        if (!MultisigHub.isMemberChange(action)) return false
        val said = title(action)?.takeIf { it.isNotBlank() } ?: return false
        if (row.optString("intent") == said) return false
        row.put("intent", said)
        return true
    }
}
