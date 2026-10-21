package app.getvela.wallet.feature.agent

import app.getvela.wallet.feature.send.core.SendOpenParams
import org.json.JSONObject

/**
 * What only the navigation host can do for the agent (spec 104 phase 7): say
 * which page is in front, open a screen, open a send. The host owns that state
 * — the back stack, the section, the flows a route keeps — so it registers
 * these while it is composed. Called on the main thread.
 */
class AgentPage(
    /** The page in front: `wallet`, `contacts`, `explore`, `settings`, or `welcome` while nobody is in. */
    val name: () -> String,
    /** Open an intent's screen (`Screen` JSON) for the person; a refusal, in words, when it cannot be shown. */
    val show: (screen: JSONObject) -> String?,
    /** A send, opened as every entry into Send opens one, on [SendOpenParams]. */
    val openSend: (params: SendOpenParams) -> Unit,
)
