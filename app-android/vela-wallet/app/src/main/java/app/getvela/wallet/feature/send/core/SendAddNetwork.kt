package app.getvela.wallet.feature.send.core

import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.settings.core.NetWizardErrorKind
import app.getvela.wallet.feature.settings.core.NetWizardPhase
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first

/**
 * Send's "add this network" (`SendOperation.AddNetwork`), answered by the
 * network machine's own verdict — the moment there is one.
 *
 * The port used to ask the network machine to add the chain and then wait for
 * ONE thing: the chain turning up as added, for up to ten seconds. Every
 * other ending — the catalog has no such chain, the chain has no P-256
 * verifier, its contracts are missing, its RPC did not answer — was
 * invisible to it, so each sat out the whole ten seconds and was then
 * reported as "not found", whatever had happened (PR 3 final notes F6 / F27).
 *
 * [settled] reads the machine's view instead: added is added; an unknown
 * chain is not found; anything else the wizard stopped for is the send
 * machine's "not compatible", carrying WHY in the core's own sentence
 * (`NetWizardView.error_key`, resolved by the caller). [await] waits on the
 * machine's COMMITS — not on a clock — for the first view that is one of
 * those. The desktop's `add_network_settled`, on the phone.
 */
object SendAddNetwork {
    /**
     * Where the network machine has got to with [chainId], as the send
     * machine words it — `null` while it is still resolving and probing.
     * [sentence] resolves a corpus key.
     */
    fun settled(view: NetView, chainId: Long, sentence: (String) -> String): SendAddNetworkOutcome? {
        val listed = view.networks.any { it.chain_id == chainId }
        val wizard = view.wizard
        if (wizard.phase == NetWizardPhase.Error) {
            return when (wizard.error) {
                is NetWizardErrorKind.NotFound -> SendAddNetworkOutcome.NotFound
                // "Already added" is the answer Send was after.
                is NetWizardErrorKind.AlreadyAdded -> if (listed) SendAddNetworkOutcome.Added else SendAddNetworkOutcome.Error
                null -> SendAddNetworkOutcome.Error
                // Refused, no RPC listed, or a check that reached no verdict:
                // not added, and the core's sentence says which.
                else -> SendAddNetworkOutcome.NotCompatible(detail = wizard.error_key?.let(sentence))
            }
        }
        if (view.last_added_chain_id == chainId && listed) return SendAddNetworkOutcome.Added
        // The scan path saves and clears the wizard in one move; a wizard
        // that is idle with nothing saved was reset under the attempt.
        if (wizard.phase == NetWizardPhase.Idle) return if (listed) SendAddNetworkOutcome.Added else SendAddNetworkOutcome.Error
        return null
    }

    /**
     * The outcome of the attempt whose event the machine has [applied]:
     * suspends until a view the machine COMMITTED after that event is settled
     * ([settled]), and not a moment longer. No timer: a refusal answers as
     * fast as the machine reaches it.
     */
    suspend fun await(
        commits: Flow<Long>,
        applied: () -> Boolean,
        view: () -> NetView,
        chainId: Long,
        sentence: (String) -> String,
    ): SendAddNetworkOutcome {
        var outcome: SendAddNetworkOutcome? = null
        commits.first {
            // Ask `applied` FIRST and read the view after: a true then
            // guarantees the view read is that event's or a later one.
            if (applied()) outcome = settled(view(), chainId, sentence)
            outcome != null
        }
        return outcome ?: SendAddNetworkOutcome.Error
    }
}
