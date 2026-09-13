package app.getvela.wallet.feature.browser.core

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.data.KeyValueStore

/**
 * The `explore_sites` machine's two arms (spec 044 T017): the one document
 * (`vela.explore`) in the shape the web and the desktop read.
 */
class ExploreExecutor(private val store: KeyValueStore) {
    suspend fun perform(operation: ExploreOperation): ExploreShellResult = when (operation) {
        ExploreOperation.ReadExplore -> ExploreShellResult.Loaded(
            store.read(KEY)?.let { raw -> runCatching { Wire.json.decodeFromString(ExploreDoc.serializer(), raw) }.getOrNull() },
        )
        is ExploreOperation.WriteExplore -> {
            store.write(KEY, Wire.json.encodeToString(ExploreDoc.serializer(), operation.doc))
            ExploreShellResult.Written
        }
    }

    fun neutralAnswer(operation: ExploreOperation): ExploreShellResult = when (operation) {
        ExploreOperation.ReadExplore -> ExploreShellResult.Loaded(null)
        is ExploreOperation.WriteExplore -> ExploreShellResult.Written
    }

    companion object {
        const val KEY = "vela.explore"
    }
}

/**
 * The `browser_history` machine's three arms (spec 044 T017): recents in
 * `vela.browserHistory`, the desktop's key and row shape.
 */
class BhistExecutor(
    private val store: KeyValueStore,
    /** The store has answered once: the controller may record its first visit. */
    private val onLoaded: () -> Unit = {},
) {
    suspend fun perform(operation: BhistOperation): BhistShellResult = when (operation) {
        BhistOperation.ReadHistory -> BhistShellResult.Loaded(
            store.read(KEY)?.let { raw ->
                runCatching { Wire.json.decodeFromString(kotlinx.serialization.builtins.ListSerializer(BhistEntry.serializer()), raw) }.getOrNull()
            }.orEmpty(),
        ).also { onLoaded() }
        is BhistOperation.WriteHistory -> {
            store.write(KEY, Wire.json.encodeToString(kotlinx.serialization.builtins.ListSerializer(BhistEntry.serializer()), operation.entries))
            BhistShellResult.Written
        }
        BhistOperation.RemoveHistory -> {
            store.write(KEY, "[]")
            BhistShellResult.Written
        }
    }

    fun neutralAnswer(operation: BhistOperation): BhistShellResult = when (operation) {
        BhistOperation.ReadHistory -> BhistShellResult.Loaded().also { onLoaded() }
        is BhistOperation.WriteHistory -> BhistShellResult.Written
        BhistOperation.RemoveHistory -> BhistShellResult.Written
    }

    companion object {
        const val KEY = "vela.browserHistory"
    }
}
