package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.diagnostics.BugReport
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * The report in flight and how it ended (spec 078 round 3; the founder:
 * 「反馈成功或失败都要有提示吧，而不是生硬的退出到设置页面吧」).
 *
 * A report ALWAYS ends in something the person sees:
 *
 * - the sheet is open when the answer arrives → the sheet shows it (filed, or
 *   the fallback block) — [State.outcome];
 * - the person closed the sheet while it was sending → the send carries on
 *   (it runs here, on the app's scope, not on the sheet's) and the answer
 *   comes up as a notice on the settings page — [State.notice], taken once by
 *   [noticeShown].
 *
 * Lives in the app container, so leaving the settings tab mid-send loses
 * neither the send nor its answer.
 */
class FeedbackReporter(
    private val scope: CoroutineScope,
    private val send: suspend (BugReport.Payload) -> BugReport.Outcome = { BugReport.send(it) },
) {
    data class State(
        val sheetOpen: Boolean = false,
        val sending: Boolean = false,
        /** What the open sheet shows. */
        val outcome: BugReport.Outcome? = null,
        /** An answer that arrived with the sheet closed: the page's notice, until shown. */
        val notice: BugReport.Outcome? = null,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state.asStateFlow()

    fun sheetOpened() = update { it.copy(sheetOpen = true) }

    /** Closed: a finished answer is done with; one still coming is owed a notice. */
    fun sheetClosed() = update { it.copy(sheetOpen = false, outcome = if (it.sending) it.outcome else null) }

    /** Send it; refused (false) while another is in flight. */
    fun submit(payload: BugReport.Payload): Boolean {
        while (true) {
            val current = _state.value
            if (current.sending) return false
            if (_state.compareAndSet(current, current.copy(sending = true, outcome = null, notice = null))) break
        }
        VelaLog.event("feedback", "send", "typed" to payload.what.length, "screenshots" to (payload.screenshots?.size ?: 0))
        scope.launch {
            val outcome = send(payload)
            VelaLog.event("feedback", "outcome", "filed" to (outcome is BugReport.Outcome.Filed), "sheetOpen" to _state.value.sheetOpen)
            update { s ->
                if (s.sheetOpen) s.copy(sending = false, outcome = outcome) else s.copy(sending = false, outcome = null, notice = outcome)
            }
        }
        return true
    }

    /** The page has shown the notice. */
    fun noticeShown() = update { it.copy(notice = null) }

    private inline fun update(transform: (State) -> State) {
        while (true) {
            val current = _state.value
            if (_state.compareAndSet(current, transform(current))) return
        }
    }
}
