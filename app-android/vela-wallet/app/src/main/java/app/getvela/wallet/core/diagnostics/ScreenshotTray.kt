package app.getvela.wallet.core.diagnostics

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Job
import kotlinx.coroutines.joinAll
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/**
 * The report's screenshots, in tile order (spec 078 round 3) — the rules with
 * no Android in them, so the JVM tests can hold them:
 *
 * - at most [max]: picking beyond it takes the first ones that fit and says
 *   [Notice.Limit] (the document picker a phone without the system photo
 *   picker falls back to does not honour a maximum);
 * - each tile is a placeholder until [prepare] answers; an answer of null
 *   removes it and says [Notice.Unsupported];
 * - a notice stands until the next change — an add or a remove;
 * - 发送 while a tile is still being prepared WAITS for it ([settled]) — an
 *   image the person saw added is never silently left out.
 *
 * [S] is what was picked (a `Uri` in the app), [R] what [prepare] made of it.
 */
class ScreenshotTray<S, R : Any>(
    private val scope: CoroutineScope,
    private val max: Int = BugReport.MAX_SCREENSHOTS,
    private val prepare: suspend (S) -> R?,
) {
    /** One square: [ready] is null while its image is still being prepared. */
    data class Tile<R>(val id: Long, val ready: R?)

    enum class Notice { Limit, Unsupported }

    private var nextId = 0L
    private val inFlight = mutableMapOf<Long, Job>()
    private val _tiles = MutableStateFlow<List<Tile<R>>>(emptyList())
    val tiles: StateFlow<List<Tile<R>>> = _tiles.asStateFlow()
    private val _notice = MutableStateFlow<Notice?>(null)
    val notice: StateFlow<Notice?> = _notice.asStateFlow()

    /** Slots left — what the picker may be asked for. */
    val room: Int get() = (max - _tiles.value.size).coerceAtLeast(0)

    fun add(picked: List<S>) {
        if (picked.isEmpty()) return
        _notice.value = null
        val taken = picked.take(room)
        if (taken.size < picked.size) _notice.value = Notice.Limit
        taken.forEach { source ->
            val id = nextId++
            _tiles.update { it + Tile(id, null) }
            // Lazy, so the job is on the books before it can finish.
            val job = scope.launch(start = CoroutineStart.LAZY) {
                val made = prepare(source)
                if (made == null) {
                    _tiles.update { tiles -> tiles.filterNot { it.id == id } }
                    _notice.value = Notice.Unsupported
                } else {
                    // Removed while it was being prepared: stays removed.
                    _tiles.update { tiles -> tiles.map { if (it.id == id) it.copy(ready = made) else it } }
                }
                inFlight.remove(id)
            }
            inFlight[id] = job
            job.start()
        }
    }

    fun remove(id: Long) {
        _tiles.update { tiles -> tiles.filterNot { it.id == id } }
        _notice.value = null
    }

    /** The prepared ones, in tile order, as they stand this instant. */
    fun ready(): List<R> = _tiles.value.mapNotNull { it.ready }

    /** Whether any tile is still being prepared. */
    val preparing: Boolean get() = _tiles.value.any { it.ready == null }

    /**
     * What 发送 carries: every tile, once each has been prepared — waiting for
     * the ones still in flight (the button shows it is busy meanwhile). One
     * that fails is taken out and named ([Notice.Unsupported]); the rest go,
     * in tile order.
     */
    suspend fun settled(): List<R> {
        while (true) {
            val pending = inFlight.values.filter { it.isActive }
            if (pending.isEmpty()) break
            pending.joinAll()
        }
        return ready()
    }
}
