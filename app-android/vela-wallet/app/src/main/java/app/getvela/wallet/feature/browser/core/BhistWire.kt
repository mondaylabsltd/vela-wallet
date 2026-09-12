package app.getvela.wallet.feature.browser.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/* The `browser_history` machine's wire (spec 044). `last_visited_ms` is `f64`. */

@Serializable
data class BhistEntry(
    val origin: String,
    val url: String,
    val host: String,
    val title: String = "",
    val favicon: String = "",
    val last_visited_ms: Double,
)

@Serializable
data class BhistView(val entries: List<BhistEntry> = emptyList())

@Serializable
sealed class BhistOperation {
    @Serializable
    @SerialName("read_history")
    data object ReadHistory : BhistOperation()

    @Serializable
    @SerialName("write_history")
    data class WriteHistory(val entries: List<BhistEntry> = emptyList()) : BhistOperation()

    @Serializable
    @SerialName("remove_history")
    data object RemoveHistory : BhistOperation()
}

@Serializable
sealed class BhistShellResult {
    @Serializable
    @SerialName("loaded")
    data class Loaded(val entries: List<BhistEntry> = emptyList()) : BhistShellResult()

    @Serializable
    @SerialName("written")
    data object Written : BhistShellResult()
}

@Serializable
sealed class BhistEvent {
    @Serializable
    @SerialName("start")
    data object Start : BhistEvent()

    @Serializable
    @SerialName("visit_recorded")
    data class VisitRecorded(val url: String, val title: String? = null, val favicon: String? = null, val now_ms: Double) : BhistEvent()

    @Serializable
    @SerialName("delete_origin")
    data class DeleteOrigin(val origin: String) : BhistEvent()

    @Serializable
    @SerialName("clear_all")
    data object ClearAll : BhistEvent()
}
