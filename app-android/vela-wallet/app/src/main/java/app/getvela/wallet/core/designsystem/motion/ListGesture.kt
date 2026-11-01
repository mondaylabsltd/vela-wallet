package app.getvela.wallet.core.designsystem.motion

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.staticCompositionLocalOf
import kotlinx.coroutines.flow.distinctUntilChanged

/**
 * r10 follow-up (core §9.4): who hears that a person is scrolling one of the
 * wallet's lists — `true` when a scroll, fling or drag starts, `false` once
 * it has settled. The composition root points it at the activity feed's
 * `interacting {on}`, so an incoming scan that comes due mid-gesture waits
 * for the list to stop. A screen with no root above it (a gallery, a test)
 * reports to nobody.
 */
val LocalListGesture = staticCompositionLocalOf<(Boolean) -> Unit> { {} }

/**
 * Report [inProgress] (a `ScrollState`'s or a `LazyListState`'s
 * `isScrollInProgress`) to [LocalListGesture] as it changes. Read in a
 * snapshot flow, so a scroll recomposes nothing here; a list that leaves the
 * screen mid-fling says it has settled, so the core never holds a scan for a
 * gesture nobody is making.
 */
@Composable
fun ReportListGesture(inProgress: () -> Boolean) {
    val report by rememberUpdatedState(LocalListGesture.current)
    val said = remember { BooleanArray(1) }
    LaunchedEffect(Unit) {
        snapshotFlow(inProgress).distinctUntilChanged().collect { on ->
            // The first frame's "not scrolling" is news to nobody.
            if (on != said[0]) {
                said[0] = on
                report(on)
            }
        }
    }
    DisposableEffect(Unit) {
        onDispose {
            if (said[0]) {
                said[0] = false
                report(false)
            }
        }
    }
}
